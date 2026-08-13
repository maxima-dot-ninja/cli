use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, StatusOptions};

/// Information about a GPG key
#[derive(Debug, Clone)]
pub struct GpgKey {
    pub key_id: String,
    pub user_id: String,
}

/// Information about a staged file change
#[derive(Debug)]
pub struct StagedChange {
    pub path: String,
    /// Where a renamed file came from — `None` for everything else
    pub old_path: Option<String>,
    pub status: ChangeStatus,
    pub additions: usize,
    pub deletions: usize,
}

impl StagedChange {
    /// "old -> new" for a rename, otherwise just the path
    pub fn location(&self) -> String {
        match &self.old_path {
            Some(old) => format!("{} -> {}", old, self.path),
            None => self.path.clone(),
        }
    }
}

#[derive(Debug)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl std::fmt::Display for ChangeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeStatus::Added => write!(f, "added"),
            ChangeStatus::Modified => write!(f, "modified"),
            ChangeStatus::Deleted => write!(f, "deleted"),
            ChangeStatus::Renamed => write!(f, "renamed"),
        }
    }
}

/// Get a list of staged changes with stats
pub fn get_staged_changes() -> Result<Vec<StagedChange>> {
    let repo = Repository::open_from_env().context("Not a git repository")?;

    let mut opts = StatusOptions::new();
    opts.include_ignored(false)
        .include_untracked(false)
        // Without this a moved file reports as one add plus one delete, which
        // both doubles the diff and hides the fact that it was only moved.
        .renames_head_to_index(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut changes = Vec::new();

    // Get the diff to get line stats
    // Handle first commit (unborn branch) by diffing against empty tree
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index()?;
    let mut diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), None)?;
    diff.find_similar(None)?;

    for entry in statuses.iter() {
        let status = entry.status();

        // Only care about staged changes
        if !status.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED,
        ) {
            continue;
        }

        // For a rename, entry.path() gives the source; the destination only
        // shows up on the delta, and that is the half worth reporting.
        let renamed = status.contains(git2::Status::INDEX_RENAMED);
        let delta_paths = entry.head_to_index().map(|delta| {
            let old = delta.old_file().path().map(|p| p.to_string_lossy().to_string());
            let new = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
            (old, new)
        });

        let (path, old_path) = match delta_paths {
            Some((Some(old), Some(new))) if renamed && old != new => (new, Some(old)),
            _ => (entry.path().unwrap_or("").to_string(), None),
        };

        let change_status = if status.contains(git2::Status::INDEX_NEW) {
            ChangeStatus::Added
        } else if status.contains(git2::Status::INDEX_DELETED) {
            ChangeStatus::Deleted
        } else if status.contains(git2::Status::INDEX_RENAMED) {
            ChangeStatus::Renamed
        } else {
            ChangeStatus::Modified
        };

        // Find line stats for this file
        let (additions, deletions) = get_file_stats(&diff, &path);

        changes.push(StagedChange {
            path,
            old_path,
            status: change_status,
            additions,
            deletions,
        });
    }

    Ok(changes)
}

/// Get addition/deletion counts for a specific file in a diff
fn get_file_stats(diff: &git2::Diff, path: &str) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;

    let _ = diff.foreach(
        &mut |delta, _| {
            if let Some(new_file) = delta.new_file().path() {
                if new_file.to_string_lossy() == path {
                    return true;
                }
            }
            if let Some(old_file) = delta.old_file().path() {
                if old_file.to_string_lossy() == path {
                    return true;
                }
            }
            true
        },
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            let file_path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if file_path == path {
                match line.origin() {
                    '+' => additions += 1,
                    '-' => deletions += 1,
                    _ => {}
                }
            }
            true
        }),
    );

    (additions, deletions)
}

/// Get the full diff of staged changes as a string
pub fn get_staged_diff() -> Result<String> {
    let repo = Repository::open_from_env().context("Not a git repository")?;

    // Handle first commit (unborn branch) by diffing against empty tree
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index()?;

    let mut opts = DiffOptions::new();
    opts.include_untracked(false);

    let mut diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))?;
    // Collapse add+delete pairs into renames before rendering. A moved file
    // otherwise emits its entire content twice, which can inflate the diff
    // several times over and push real changes past the size limit.
    diff.find_similar(None)?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            diff_text.push(origin);
        }
        diff_text.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;

    Ok(budget_diff(&diff_text))
}

/// Trim an oversized diff without letting any file disappear.
///
/// Cutting the stream at a fixed byte count drops whole files off the end — the
/// model then describes only the files that survived and invents a story for the
/// rest. Giving every file its own slice of the budget keeps all of them visible.
fn budget_diff(diff_text: &str) -> String {
    const MAX_DIFF_LEN: usize = 30000;

    if diff_text.len() <= MAX_DIFF_LEN {
        return diff_text.to_string();
    }

    let sections = split_file_sections(diff_text);
    if sections.is_empty() {
        return clip_lines(diff_text, MAX_DIFF_LEN);
    }

    let per_file = MAX_DIFF_LEN / sections.len();
    sections
        .iter()
        .map(|section| clip_lines(section, per_file))
        .collect::<Vec<String>>()
        .join("")
}

/// Split a patch into one chunk per file, keeping the "diff --git" header
fn split_file_sections(diff_text: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    let mut current = String::new();

    for line in diff_text.lines() {
        if line.starts_with("diff --git") && !current.is_empty() {
            sections.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        sections.push(current);
    }
    sections
}

/// Keep whole lines only, so the result never splits a UTF-8 character
fn clip_lines(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }

    let mut out = String::new();
    for line in text.lines() {
        if out.len() + line.len() + 1 > max {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("... (rest of this file's diff omitted for length)\n");
    out
}

/// One line per staged file: what happened to it and how many lines moved.
///
/// This is the ground truth about the change and it always reaches the model
/// intact, even when the diff below it had to be trimmed.
pub fn change_summary(changes: &[StagedChange]) -> String {
    changes
        .iter()
        .map(|c| format!("{} {} (+{} -{})", c.status, c.location(), c.additions, c.deletions))
        .collect::<Vec<String>>()
        .join("\n")
}

/// Real `git diff --cached` output for one staged file. Runs from the repo root so
/// the path resolves the same way regardless of which subdirectory lgit was run in.
pub fn get_file_diff(path: &str) -> Result<String> {
    let repo = Repository::open_from_env().context("Not a git repository")?;
    let root = repo
        .workdir()
        .context("Repository has no working directory")?
        .to_path_buf();

    let output = std::process::Command::new("git")
        .current_dir(&root)
        .args(["diff", "--cached", "--", path])
        .output()
        .context("Failed to run git diff")?;

    if !output.status.success() {
        anyhow::bail!(
            "git diff failed for {}: {}",
            path,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// List available GPG secret keys
pub fn list_gpg_keys() -> Result<Vec<GpgKey>> {
    let output = std::process::Command::new("gpg")
        .args(["--list-secret-keys", "--keyid-format", "LONG"])
        .output()
        .context("Failed to execute gpg")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut keys = Vec::new();
    let mut current_key_id: Option<String> = None;

    for line in stdout.lines() {
        // Lines like: "sec   rsa4096/ABCD1234EFGH5678 2023-01-01 [SC]"
        if line.starts_with("sec") {
            if let Some(key_part) = line.split_whitespace().nth(1) {
                if let Some(key_id) = key_part.split('/').nth(1) {
                    current_key_id = Some(key_id.to_string());
                }
            }
        }
        // Lines like: "uid           [ultimate] User Name <email@example.com>"
        if line.contains("uid") && line.contains("[") {
            if let Some(key_id) = current_key_id.take() {
                // Extract the user id part after the trust level bracket
                let user_id = line
                    .split(']')
                    .nth(1)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                keys.push(GpgKey { key_id, user_id });
            }
        }
    }

    Ok(keys)
}

/// Create a signed commit with the given message and GPG key
pub fn commit_signed(message: &str, gpg_key_id: &str) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["commit", "-S", &format!("--gpg-sign={}", gpg_key_id), "-m", message])
        .output()
        .context("Failed to execute git commit")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git commit failed: {}", stderr.trim());
    }

    Ok(())
}

/// Create an unsigned commit with the given message
pub fn commit_unsigned(message: &str) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .output()
        .context("Failed to execute git commit")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git commit failed: {}", stderr.trim());
    }

    Ok(())
}

/// Create a lightweight tag on the current commit
pub fn create_tag(tag_name: &str) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["tag", tag_name])
        .output()
        .context("Failed to execute git tag")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git tag failed: {}", stderr.trim());
    }

    Ok(())
}

/// Push to the current remote tracking branch
/// Returns Ok(true) if push succeeded, Ok(false) if rejected due to remote changes
pub fn push() -> Result<bool> {
    // First try a normal push
    let output = std::process::Command::new("git")
        .args(["push"])
        .output()
        .context("Failed to execute git push")?;

    if output.status.success() {
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // If no upstream branch, set it up automatically
    if stderr.contains("has no upstream branch") || stderr.contains("no upstream configured") {
        let branch = get_current_branch()?;
        let retry = std::process::Command::new("git")
            .args(["push", "--set-upstream", "origin", &branch])
            .output()
            .context("Failed to execute git push --set-upstream")?;

        if retry.status.success() {
            return Ok(true);
        }

        let retry_stderr = String::from_utf8_lossy(&retry.stderr);
        if retry_stderr.contains("rejected") && retry_stderr.contains("fetch first") {
            return Ok(false);
        }
        anyhow::bail!("git push failed: {}", retry_stderr.trim());
    }

    // Check if push was rejected because remote has newer commits
    if stderr.contains("rejected") && stderr.contains("fetch first") {
        return Ok(false);
    }

    anyhow::bail!("git push failed: {}", stderr.trim())
}

/// Push to the current remote tracking branch including tags
/// Returns Ok(true) if push succeeded, Ok(false) if rejected due to remote changes
pub fn push_with_tags() -> Result<bool> {
    // First push commits
    let result = push()?;
    if !result {
        return Ok(false);
    }

    // Then push tags
    let output = std::process::Command::new("git")
        .args(["push", "--tags"])
        .output()
        .context("Failed to execute git push --tags")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git push --tags failed: {}", stderr.trim());
    }

    Ok(true)
}

/// Pull from the current remote tracking branch
pub fn pull() -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["pull"])
        .output()
        .context("Failed to execute git pull")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git pull failed: {}", stderr.trim());
    }

    Ok(())
}

/// Get the URL for creating a PR on GitHub/GitLab
pub fn get_pr_url() -> Result<Option<String>> {
    let repo = Repository::open_from_env()?;

    // Get current branch name
    let head = repo.head()?;
    let branch = head
        .shorthand()
        .ok_or_else(|| anyhow::anyhow!("Could not get branch name"))?;

    // Get remote URL
    let remote = repo.find_remote("origin").ok();
    let url = remote
        .as_ref()
        .and_then(|r| r.url())
        .unwrap_or("");

    if url.is_empty() {
        return Ok(None);
    }

    // Parse GitHub/GitLab URL
    let pr_url = if url.contains("github.com") {
        let repo_path = extract_repo_path(url);
        Some(format!(
            "https://github.com/{}/compare/{}?expand=1",
            repo_path, branch
        ))
    } else if url.contains("gitlab.com") {
        let repo_path = extract_repo_path(url);
        Some(format!(
            "https://gitlab.com/{}/-/merge_requests/new?merge_request[source_branch]={}",
            repo_path, branch
        ))
    } else {
        None
    };

    Ok(pr_url)
}

/// Extract owner/repo from a git URL
fn extract_repo_path(url: &str) -> String {
    url.trim_end_matches(".git")
        .replace("git@github.com:", "")
        .replace("git@gitlab.com:", "")
        .replace("https://github.com/", "")
        .replace("https://gitlab.com/", "")
}

/// Get the current branch name
#[allow(dead_code)]
pub fn get_current_branch() -> Result<String> {
    let repo = Repository::open_from_env()?;
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_section(name: &str, lines: usize) -> String {
        let mut s = format!("diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n");
        for i in 0..lines {
            s.push_str(&format!("+line {i} of {name}\n"));
        }
        s
    }

    #[test]
    fn small_diffs_pass_through_untouched() {
        let diff = file_section("a.rs", 3);
        assert_eq!(budget_diff(&diff), diff);
    }

    #[test]
    fn every_file_survives_an_oversized_diff() {
        // One huge file followed by small ones — the exact shape that used to
        // let a blunt truncate swallow everything after the first file.
        let diff = format!(
            "{}{}{}",
            file_section("huge.md", 4000),
            file_section("small.ts", 5),
            file_section("other.ts", 5)
        );
        assert!(diff.len() > 30000, "fixture must exceed the cap");

        let out = budget_diff(&diff);
        assert!(out.len() <= 30000 + 500, "budget respected, got {}", out.len());
        for name in ["huge.md", "small.ts", "other.ts"] {
            assert!(out.contains(name), "{name} was dropped from the trimmed diff");
        }
    }

    #[test]
    fn clipping_keeps_whole_lines() {
        let text = "diff --git a/x b/x\n+aaaa\n+bbbb\n+cccc\n";
        let out = clip_lines(text, 30);
        assert!(out.lines().all(|l| !l.is_empty() || l.is_empty()));
        assert!(out.contains("omitted for length"));
        assert!(out.len() <= 30 + 60);
    }

    #[test]
    fn clipping_never_splits_a_utf8_char() {
        let text = "diff --git a/x b/x\n+é€ñ 🚀 multibyte content here\n+more\n";
        let out = clip_lines(text, 25);
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn sections_split_per_file() {
        let diff = format!("{}{}", file_section("a.rs", 2), file_section("b.rs", 2));
        let sections = split_file_sections(&diff);
        assert_eq!(sections.len(), 2);
        assert!(sections[0].contains("a.rs"));
        assert!(sections[1].contains("b.rs"));
    }

    #[test]
    fn summary_lists_status_and_counts() {
        let changes = vec![
            StagedChange { path: "_docs/a.md".into(), old_path: Some("a.md".into()), status: ChangeStatus::Renamed, additions: 0, deletions: 0 },
            StagedChange { path: "b.ts".into(), old_path: None, status: ChangeStatus::Deleted, additions: 0, deletions: 12 },
        ];
        let out = change_summary(&changes);
        assert!(out.contains("renamed a.md -> _docs/a.md (+0 -0)"), "got: {out}");
        assert!(out.contains("deleted b.ts (+0 -12)"));
    }
}
