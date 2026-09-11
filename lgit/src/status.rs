//! `lgit status`: gather everything git knows about the working state, then
//! have the model explain it the way a teammate glancing at your screen would.

use crate::{ai, config, git, ui};
use anyhow::{Context, Result};
use git2::{Repository, RepositoryState};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// The staged and unstaged diffs get separate budgets, so a large unstaged
/// refactor can't push the staged change out of the model's view.
const STAGED_BUDGET: usize = 20_000;
const UNSTAGED_BUDGET: usize = 20_000;
/// Characters of untracked-file content sent in total, and per file
const UNTRACKED_BUDGET: usize = 8_000;
const UNTRACKED_PER_FILE: usize = 1_500;
/// Files named inside one untracked directory before the rest are only counted
const DIR_LISTING: usize = 15;
/// Lines of the file list before the rest are only counted
const FILE_LISTING: usize = 150;

/// Untracked files whose names contain one of these never have their contents
/// sent. The name alone is enough for the model to warn about it.
const SECRET_MARKERS: &[&str] = &[
    ".env", ".pem", ".key", ".p12", ".pfx", "id_rsa", "id_ed25519", "credential", "secret",
    ".npmrc", ".netrc",
];

/// Where to look for the branch this one was cut from, in order of preference
const BASE_CANDIDATES: &[&str] = &["origin/HEAD", "origin/main", "origin/master", "main", "master"];

/// What each unmerged code in `git status --porcelain=v2` means
const CONFLICTS: &[(&str, &str)] = &[
    ("UU", "edited on both sides"),
    ("AA", "added on both sides"),
    ("DD", "deleted on both sides"),
    ("AU", "added by us"),
    ("UA", "added by them"),
    ("DU", "deleted by us"),
    ("UD", "deleted by them"),
];

const AGE_UNITS: &[(u64, &str)] = &[
    (365 * 86_400, "year"),
    (30 * 86_400, "month"),
    (7 * 86_400, "week"),
    (86_400, "day"),
    (3_600, "hour"),
    (60, "minute"),
];

const SIZE_UNITS: &[(u64, &str)] = &[(1 << 30, "GB"), (1 << 20, "MB"), (1 << 10, "KB")];

/// Print the plain facts straight away, then the model's explanation. Read-only:
/// nothing here changes the repository.
pub async fn run() -> Result<()> {
    let cfg = config::load_config()?;
    let snapshot = Snapshot::read()?;

    ui::print_status_header(&snapshot.name, &[snapshot.branch_line(), snapshot.counts_line()]);

    if snapshot.is_quiet() {
        ui::print_success("Nothing is going on here: no changes, nothing to push or pull, and no stashes.");
        return Ok(());
    }

    let spinner = ui::create_spinner("Reading diffs and new files...");
    let facts = snapshot.facts();
    spinner.set_message(format!("Asking {} what's going on...", cfg.provider.model));
    let report = ai::explain_status(&cfg, &facts).await;
    spinner.finish_and_clear();

    ui::print_status_report(&report?);
    Ok(())
}

/// The `# branch.*` and `# stash` headers of porcelain v2
#[derive(Default)]
struct Branch {
    /// Branch name, or "(detached)"
    head: String,
    /// Commit id, or "(initial)" before the first commit
    oid: String,
    upstream: Option<String>,
    /// Ahead and behind the upstream. Missing when the upstream ref is gone.
    sync: Option<(usize, usize)>,
    stashes: usize,
}

impl Branch {
    fn ahead(&self) -> usize {
        self.sync.map_or(0, |(ahead, _)| ahead)
    }

    fn behind(&self) -> usize {
        self.sync.map_or(0, |(_, behind)| behind)
    }

    fn read_header(&mut self, header: &str) {
        let Some((key, value)) = header.split_once(' ') else {
            return;
        };
        match key {
            "branch.oid" => self.oid = value.to_string(),
            "branch.head" => self.head = value.to_string(),
            "branch.upstream" => self.upstream = Some(value.to_string()),
            "branch.ab" => self.sync = parse_ab(value),
            "stash" => self.stashes = value.parse().unwrap_or(0),
            _ => {}
        }
    }
}

/// One path from `git status`
struct Entry {
    path: String,
    /// Where a renamed or copied file came from
    orig: Option<String>,
    kind: Kind,
}

enum Kind {
    /// Index and worktree status letters; '.' means untouched on that side
    Changed { staged: char, unstaged: char },
    Conflict(String),
    Untracked,
}

/// Everything read from the repository up front. The diffs are read later, in
/// `facts`, because a clean repo never needs them.
struct Snapshot {
    root: PathBuf,
    name: String,
    branch: Branch,
    entries: Vec<Entry>,
    /// A merge, rebase, cherry-pick, revert, or bisect that hasn't finished
    operation: Option<String>,
    /// How long ago the remote was last fetched, when it ever was
    fetched: Option<String>,
}

impl Snapshot {
    fn read() -> Result<Self> {
        let repo = Repository::open_from_env().context("Not a git repository")?;
        let root = repo
            .workdir()
            .context("This is a bare repository, so there is no working tree to describe")?
            .to_path_buf();
        let raw = git(&root, &["status", "--porcelain=v2", "--branch", "--show-stash", "-z"])
            .context("git status failed")?;
        let (branch, entries) = parse_porcelain(&raw);

        Ok(Self {
            name: crate::repo_name(&root),
            operation: operation(&repo, &root),
            fetched: fetch_age(&root),
            root,
            branch,
            entries,
        })
    }

    fn is_quiet(&self) -> bool {
        self.entries.is_empty()
            && self.operation.is_none()
            && self.branch.ahead() == 0
            && self.branch.behind() == 0
            && self.branch.stashes == 0
    }

    fn branch_line(&self) -> String {
        let b = &self.branch;
        if b.head == "(detached)" {
            return format!("Detached HEAD at {}.", short(&b.oid));
        }
        let Some(upstream) = &b.upstream else {
            return format!("On {}, which has no upstream yet.", b.head);
        };
        let Some((ahead, behind)) = b.sync else {
            return format!("On {}, tracking {upstream}, which no longer exists on the remote.", b.head);
        };
        let sync = match (ahead, behind) {
            (0, 0) => "up to date".to_string(),
            (a, 0) => format!("{a} ahead"),
            (0, z) => format!("{z} behind"),
            (a, z) => format!("{a} ahead and {z} behind"),
        };
        // No FETCH_HEAD only means nothing was ever fetched; pushes still keep
        // the tracking ref current, so it says nothing about how stale it is.
        let fetched = self
            .fetched
            .as_ref()
            .map(|age| format!(" (last fetched {age} ago)"))
            .unwrap_or_default();
        format!("On {}, tracking {upstream}: {sync}{fetched}.", b.head)
    }

    fn counts_line(&self) -> String {
        let counts = [
            (self.count(|k| matches!(k, Kind::Conflict(_))), "conflicted"),
            (self.count(|k| matches!(k, Kind::Changed { staged, .. } if *staged != '.')), "staged"),
            (self.count(|k| matches!(k, Kind::Changed { unstaged, .. } if *unstaged != '.')), "unstaged"),
            (self.count(|k| matches!(k, Kind::Untracked)), "untracked"),
        ];
        let parts: Vec<String> = counts
            .iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, label)| format!("{n} {label}"))
            .collect();
        if parts.is_empty() {
            return "The working tree is clean.".to_string();
        }
        format!("Files: {}.", parts.join(", "))
    }

    fn count(&self, pick: impl Fn(&Kind) -> bool) -> usize {
        self.entries.iter().filter(|e| pick(&e.kind)).count()
    }

    /// The full report the model reads. Sections that don't apply are left out.
    fn facts(&self) -> String {
        let sections = [
            ("Overview", Some(self.overview())),
            ("Unfinished operation", self.operation.clone()),
            ("Compared with the base branch", self.base_comparison()),
            ("Commits not pushed yet", self.unpushed()),
            ("Commits on the upstream that this branch doesn't have, as of the last fetch", self.incoming()),
            ("Recent commits", self.log(&["-n", "8"])),
            ("Stashes", self.stashes()),
            ("Changed files", self.file_list()),
            ("Staged diff, which is exactly what the next commit would contain", self.diff(&["--cached", "-M"], STAGED_BUDGET)),
            ("Unstaged diff, edited but not staged", self.diff(&[], UNSTAGED_BUDGET)),
            ("Untracked files and how they start", self.untracked()),
        ];
        sections
            .into_iter()
            .filter_map(|(title, body)| body.map(|body| format!("## {title}\n{body}")))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn overview(&self) -> String {
        let mut lines = vec![
            format!("Repository: {}", self.name),
            self.branch_line(),
            self.counts_line(),
        ];
        if self.branch.oid == "(initial)" {
            lines.push("There are no commits yet.".to_string());
        }
        lines.join("\n")
    }

    /// How this branch relates to main (or whatever the default branch is)
    fn base_comparison(&self) -> Option<String> {
        let base = BASE_CANDIDATES
            .iter()
            .find_map(|c| git(&self.root, &["rev-parse", "--verify", "--quiet", "--abbrev-ref", c]))?;
        if base.trim_start_matches("origin/") == self.branch.head {
            return None;
        }
        let ours = self.count_commits(&format!("{base}..HEAD"));
        let theirs = self.count_commits(&format!("HEAD..{base}"));
        if ours == 0 && theirs == 0 {
            return None;
        }
        let summary = format!(
            "This branch has {ours} commit(s) that {base} doesn't, and {base} has {theirs} commit(s) that this branch doesn't."
        );
        let own = self
            .log(&["-n", "15", &format!("{base}..HEAD")])
            .map(|log| format!("\nThis branch's own commits:\n{log}"))
            .unwrap_or_default();
        Some(format!("{summary}{own}"))
    }

    fn count_commits(&self, range: &str) -> usize {
        git(&self.root, &["rev-list", "--count", range])
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }

    fn unpushed(&self) -> Option<String> {
        if self.branch.ahead() == 0 {
            return None;
        }
        self.log(&["-n", "15", "@{u}..HEAD"])
    }

    fn incoming(&self) -> Option<String> {
        if self.branch.behind() == 0 {
            return None;
        }
        self.log(&["-n", "15", "HEAD..@{u}"])
    }

    fn log(&self, extra: &[&str]) -> Option<String> {
        let mut args = vec!["log", "--no-color", "--format=%h %ar, %an: %s"];
        args.extend_from_slice(extra);
        git(&self.root, &args).filter(|out| !out.is_empty())
    }

    fn stashes(&self) -> Option<String> {
        if self.branch.stashes == 0 {
            return None;
        }
        git(&self.root, &["stash", "list", "-n", "10", "--format=%gd (%cr): %gs"])
    }

    fn file_list(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = self
            .entries
            .iter()
            .take(FILE_LISTING)
            .map(|e| self.describe(e))
            .collect();
        if self.entries.len() > FILE_LISTING {
            lines.push(format!("... and {} more files", self.entries.len() - FILE_LISTING));
        }
        Some(lines.join("\n"))
    }

    fn describe(&self, entry: &Entry) -> String {
        let name = match &entry.orig {
            Some(orig) => format!("{orig} -> {}", entry.path),
            None => entry.path.clone(),
        };
        let state = match &entry.kind {
            Kind::Changed { staged, unstaged } => sides(*staged, *unstaged),
            Kind::Conflict(xy) => format!("CONFLICT, {}", conflict_meaning(xy)),
            Kind::Untracked => "untracked, git has never seen it".to_string(),
        };
        let age = edited_ago(&self.root.join(&entry.path))
            .map(|age| format!(", last edited {age} ago"))
            .unwrap_or_default();
        format!("{name}: {state}{age}")
    }

    fn diff(&self, extra: &[&str], budget: usize) -> Option<String> {
        let mut args = vec!["-c", "core.quotepath=off", "diff", "--no-color", "--no-ext-diff"];
        args.extend_from_slice(extra);
        let text = git(&self.root, &args)?;
        (!text.is_empty()).then(|| git::budget_diff(&text, budget))
    }

    /// New files with their first lines, so the model can tell what they are.
    /// Untracked directories arrive collapsed ("dir/") and get expanded here.
    fn untracked(&self) -> Option<String> {
        let paths: Vec<&str> = self
            .entries
            .iter()
            .filter(|e| matches!(e.kind, Kind::Untracked))
            .map(|e| e.path.as_str())
            .collect();
        if paths.is_empty() {
            return None;
        }
        let mut budget = UNTRACKED_BUDGET;
        let mut out = Vec::new();
        for path in paths {
            if path.ends_with('/') {
                out.push(self.untracked_dir(path, &mut budget));
                continue;
            }
            out.push(self.preview(path, &mut budget));
        }
        Some(out.join("\n"))
    }

    fn untracked_dir(&self, dir: &str, budget: &mut usize) -> String {
        let listing = git(&self.root, &["ls-files", "--others", "--exclude-standard", "-z", "--", dir])
            .unwrap_or_default();
        let files: Vec<&str> = listing.split('\0').filter(|f| !f.is_empty()).collect();
        let mut out = vec![format!("New directory {dir} holding {} untracked file(s):", files.len())];
        out.extend(files.iter().take(DIR_LISTING).map(|f| self.preview(f, budget)));
        if files.len() > DIR_LISTING {
            out.push(format!("... and {} more files in {dir}", files.len() - DIR_LISTING));
        }
        out.join("\n")
    }

    fn preview(&self, path: &str, budget: &mut usize) -> String {
        let full = self.root.join(path);
        let size = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
        let heading = format!(">>> {path} ({})", human_size(size));
        if looks_secret(path) {
            return format!("{heading}: contents withheld because the name looks like a secret");
        }
        if *budget == 0 {
            return heading;
        }
        let max = UNTRACKED_PER_FILE.min(*budget);
        let Some(text) = read_head(&full, max) else {
            return format!("{heading}: binary or unreadable");
        };
        *budget = budget.saturating_sub(text.len());
        let cut = if size as usize > max { "\n... (rest of the file not shown)" } else { "" };
        format!("{heading}\n{}{cut}", text.trim_end())
    }
}

fn parse_porcelain(raw: &str) -> (Branch, Vec<Entry>) {
    let mut branch = Branch::default();
    let mut entries = Vec::new();
    let mut tokens = raw.split('\0').filter(|t| !t.is_empty());

    while let Some(token) = tokens.next() {
        if let Some(header) = token.strip_prefix("# ") {
            branch.read_header(header);
            continue;
        }
        let Some((tag, rest)) = token.split_once(' ') else {
            continue;
        };
        let entry = match tag {
            "1" => changed(rest, 7, None),
            // A rename carries its source path as the next NUL-separated token
            "2" => changed(rest, 8, tokens.next()),
            "u" => conflict(rest),
            "?" => Some(Entry { path: rest.to_string(), orig: None, kind: Kind::Untracked }),
            _ => None,
        };
        entries.extend(entry);
    }
    (branch, entries)
}

/// The XY code and the path, which follows `before_path` space-separated fields
fn fields(rest: &str, before_path: usize) -> Option<(&str, &str)> {
    let parts: Vec<&str> = rest.splitn(before_path + 1, ' ').collect();
    Some((*parts.first()?, *parts.get(before_path)?))
}

fn changed(rest: &str, before_path: usize, orig: Option<&str>) -> Option<Entry> {
    let (xy, path) = fields(rest, before_path)?;
    let mut codes = xy.chars();
    Some(Entry {
        path: path.to_string(),
        orig: orig.map(str::to_string),
        kind: Kind::Changed { staged: codes.next()?, unstaged: codes.next()? },
    })
}

fn conflict(rest: &str) -> Option<Entry> {
    let (xy, path) = fields(rest, 9)?;
    Some(Entry { path: path.to_string(), orig: None, kind: Kind::Conflict(xy.to_string()) })
}

/// "+2 -1" into (2, 1)
fn parse_ab(value: &str) -> Option<(usize, usize)> {
    let (ahead, behind) = value.split_once(' ')?;
    Some((
        ahead.trim_start_matches('+').parse().ok()?,
        behind.trim_start_matches('-').parse().ok()?,
    ))
}

/// "staged modified; unstaged modified (partly staged)"
fn sides(staged: char, unstaged: char) -> String {
    let text = [("staged", staged), ("unstaged", unstaged)]
        .iter()
        .filter(|(_, code)| *code != '.')
        .map(|(side, code)| format!("{side} {}", verb(*code)))
        .collect::<Vec<_>>()
        .join("; ");
    if staged != '.' && unstaged != '.' {
        return format!("{text} (partly staged)");
    }
    text
}

fn verb(code: char) -> &'static str {
    match code {
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "type changed",
        _ => "modified",
    }
}

fn conflict_meaning(xy: &str) -> &'static str {
    CONFLICTS
        .iter()
        .find(|(code, _)| *code == xy)
        .map_or("unmerged", |(_, meaning)| meaning)
}

/// Run git in the repo root and return its output. `None` when git fails,
/// which for most of these queries just means "doesn't apply here": no
/// upstream, no commits yet, no base branch.
///
/// Optional locks are off so a status read never takes the index lock and
/// can't collide with a commit or rebase running at the same moment.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

fn operation(repo: &Repository, root: &Path) -> Option<String> {
    let what = match repo.state() {
        RepositoryState::Clean => return None,
        RepositoryState::Merge => "merge",
        RepositoryState::Revert | RepositoryState::RevertSequence => "revert",
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => "cherry-pick",
        RepositoryState::Bisect => "bisect",
        RepositoryState::Rebase | RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge => "rebase",
        RepositoryState::ApplyMailbox | RepositoryState::ApplyMailboxOrRebase => "git am",
    };
    let gitdir = repo.path();
    let detail = rebase_progress(gitdir, root).or_else(|| first_line(&gitdir.join("MERGE_MSG")));
    Some(match detail {
        Some(detail) => format!("A {what} is in progress: {detail}"),
        None => format!("A {what} is in progress."),
    })
}

/// The two layouts git uses for an in-progress rebase: directory, current step file, total file
const REBASE_LAYOUTS: &[(&str, &str, &str)] = &[
    ("rebase-merge", "msgnum", "end"),
    ("rebase-apply", "next", "last"),
];

fn rebase_progress(gitdir: &Path, root: &Path) -> Option<String> {
    REBASE_LAYOUTS.iter().find_map(|(dir, step, total)| {
        let dir = gitdir.join(dir);
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok().map(|s| s.trim().to_string());
        let (step, total) = (read(step)?, read(total)?);
        let head = read("head-name").unwrap_or_default();
        let onto = read("onto")
            .and_then(|oid| git(root, &["log", "-1", "--format=%h (%s)", &oid]))
            .unwrap_or_default();
        Some(format!(
            "replaying {} onto {onto}, at commit {step} of {total}",
            head.trim_start_matches("refs/heads/")
        ))
    })
}

fn first_line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn fetch_age(root: &Path) -> Option<String> {
    let path = git(root, &["rev-parse", "--git-path", "FETCH_HEAD"])?;
    edited_ago(&root.join(path))
}

fn edited_ago(path: &Path) -> Option<String> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(ago(SystemTime::now().duration_since(modified).ok()?))
}

fn ago(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    AGE_UNITS
        .iter()
        .find(|(size, _)| secs >= *size)
        .map(|(size, unit)| {
            let n = secs / size;
            format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
        })
        .unwrap_or_else(|| "under a minute".to_string())
}

fn human_size(bytes: u64) -> String {
    SIZE_UNITS
        .iter()
        .find(|(size, _)| bytes >= *size)
        .map(|(size, unit)| format!("{:.1} {unit}", bytes as f64 / *size as f64))
        .unwrap_or_else(|| format!("{bytes} bytes"))
}

fn looks_secret(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    SECRET_MARKERS.iter().any(|marker| name.contains(marker))
}

/// The first `max` bytes of a file as text, or `None` for binary or unreadable files
fn read_head(path: &Path, max: usize) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(max as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn short(oid: &str) -> &str {
    &oid[..oid.len().min(7)]
}
