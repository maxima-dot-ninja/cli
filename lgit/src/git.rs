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
    pub status: ChangeStatus,
    pub additions: usize,
    pub deletions: usize,
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
        .include_untracked(false);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut changes = Vec::new();

    // Get the diff to get line stats
    // Handle first commit (unborn branch) by diffing against empty tree
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index()?;
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), None)?;

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

        let path = entry.path().unwrap_or("").to_string();

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

    let diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        if origin == '+' || origin == '-' || origin == ' ' {
            diff_text.push(origin);
        }
        diff_text.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;

    // Truncate if too large (keep under ~8k tokens worth)
    const MAX_DIFF_LEN: usize = 30000;
    if diff_text.len() > MAX_DIFF_LEN {
        diff_text.truncate(MAX_DIFF_LEN);
        diff_text.push_str("\n\n... (diff truncated due to size)");
    }

    Ok(diff_text)
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
