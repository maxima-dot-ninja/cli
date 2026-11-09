//! `lgit pull`: `git pull` on the branch you are on, from anywhere inside the repo. With `--root` —
//! `lgit root pull` reads the same — every repo one level below the current folder pulls at once,
//! each on its own branch, and says how it went the moment it finishes, then a summary.
//!
//! It is git's own pull, so `pull.rebase` and the rest of your git config decide how a pull lands.
//! Two things are pinned so a dozen at once can never hang: no prompt for a login (a repo that needs
//! one fails and says so), and no editor for a merge commit (git's own message is kept).

use anyhow::Result;
use console::style;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tokio::sync::mpsc;

/// How one repo's pull went.
enum Pulled {
    /// New commits came in: how many, on which branch.
    Updated { branch: String, commits: usize },
    UpToDate { branch: String },
    /// There was nothing to pull from.
    Skipped(String),
    Failed(String),
}

fn git(repo: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .output()
}

/// What a git command printed, when it worked.
fn said(repo: &Path, args: &[&str]) -> Option<String> {
    let out = git(repo, args).ok().filter(|out| out.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Git's reasons for refusing, in the words to act on. Anything else is git's own last line.
const WHY: &[(&str, &str)] = &[
    ("would be overwritten", "your uncommitted changes would be overwritten — commit or stash them first"),
    ("divergent branches", "this branch and its upstream have diverged — set pull.rebase, or pull by hand"),
    ("CONFLICT", "the pull stopped on merge conflicts — the repo is mid-merge until you resolve them"),
    ("terminal prompts disabled", "the remote asked for a login"),
    ("Authentication failed", "the remote refused the login"),
    ("Could not resolve host", "the remote could not be reached"),
    ("unmerged files", "it has unmerged files from an earlier merge — finish that first"),
];

fn reason(out: &Output) -> String {
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    if let Some((_, why)) = WHY.iter().find(|(mark, _)| text.contains(mark)) {
        return why.to_string();
    }
    text.lines().map(str::trim).filter(|line| !line.is_empty()).last().unwrap_or("git pull failed").to_string()
}

/// Pull the branch `repo` is on from its upstream.
fn pull(repo: &Path) -> Pulled {
    let Some(branch) = said(repo, &["symbolic-ref", "--short", "-q", "HEAD"]) else {
        return Pulled::Skipped("not on a branch (detached HEAD)".into());
    };
    if said(repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_none() {
        return Pulled::Skipped(format!("{branch} has no upstream to pull from"));
    }
    // None on a branch with no commits yet.
    let before = said(repo, &["rev-parse", "HEAD"]);
    let out = match git(repo, &["pull", "--no-edit"]) {
        Ok(out) => out,
        Err(problem) => return Pulled::Failed(format!("could not run git: {problem}")),
    };
    if !out.status.success() {
        return Pulled::Failed(reason(&out));
    }
    let range = before.map(|sha| format!("{sha}..HEAD")).unwrap_or_else(|| "HEAD".into());
    let commits = said(repo, &["rev-list", "--count", &range]).and_then(|n| n.parse().ok()).unwrap_or(0);
    match commits {
        0 => Pulled::UpToDate { branch },
        commits => Pulled::Updated { branch, commits },
    }
}

/// One repo's line: a mark, its name, and what happened.
fn line(name: &str, pulled: &Pulled) -> String {
    match pulled {
        Pulled::Updated { branch, commits } => {
            let noun = if *commits == 1 { "commit" } else { "commits" };
            format!("{} {}  {}", style("✓").green(), style(name).bold(), style(format!("↓ {commits} {noun} on {branch}")).green())
        }
        Pulled::UpToDate { branch } => format!("{} {}  {}", style("·").dim(), name, style(format!("up to date on {branch}")).dim()),
        Pulled::Skipped(why) => format!("{} {}  {}", style("–").yellow(), name, style(why).yellow()),
        Pulled::Failed(why) => format!("{} {}  {}", style("✗").red(), style(name).bold(), style(why).red()),
    }
}

/// `lgit pull`, in the repo around the current folder.
pub fn run() -> Result<()> {
    let here = std::env::current_dir()?;
    let Some(top) = said(&here, &["rev-parse", "--show-toplevel"]) else {
        crate::ui::print_warning("Not inside a git repository. From a folder of repos, `lgit root pull` pulls every one.");
        return Ok(());
    };
    let top = PathBuf::from(top);
    let spinner = crate::ui::create_spinner("Pulling…");
    let pulled = pull(&top);
    spinner.finish_and_clear();
    println!("{}", line(&crate::repo_name(&top), &pulled));
    if matches!(pulled, Pulled::Failed(_)) {
        std::process::exit(1);
    }
    Ok(())
}

/// `lgit root pull`: every repo in the current folder at once, each line printed as it finishes.
pub async fn run_root() -> Result<()> {
    let root = std::env::current_dir()?;
    let repos = crate::child_repos(&root)?;
    if repos.is_empty() {
        crate::ui::print_warning(&format!("No git repositories found in {}", root.display()));
        return Ok(());
    }
    println!("{}", style(format!("Pulling {} repos in {}", repos.len(), root.display())).dim());

    let (tx, mut rx) = mpsc::unbounded_channel::<(String, Pulled)>();
    for repo in repos.iter().cloned() {
        let tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            let pulled = pull(&repo);
            let _ = tx.send((crate::repo_name(&repo), pulled));
        });
    }
    drop(tx);

    let (mut updated, mut current, mut skipped, mut failed) = (0, 0, 0, 0);
    while let Some((name, pulled)) = rx.recv().await {
        println!("{}", line(&name, &pulled));
        match pulled {
            Pulled::Updated { .. } => updated += 1,
            Pulled::UpToDate { .. } => current += 1,
            Pulled::Skipped(_) => skipped += 1,
            Pulled::Failed(_) => failed += 1,
        }
    }

    let parts = [(updated, "updated"), (current, "up to date"), (skipped, "skipped"), (failed, "failed")];
    let summary: Vec<String> = parts.iter().filter(|(n, _)| *n > 0).map(|(n, what)| format!("{n} {what}")).collect();
    println!("{}", style(summary.join(" · ")).bold());
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
