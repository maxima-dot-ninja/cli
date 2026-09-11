mod ai;
mod config;
mod git;
mod providers;
mod setup;
mod status;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "lgit")]
#[command(about = "AI-powered git commits", long_about = None)]
#[command(version)]
struct Cli {
    /// Re-run the setup wizard
    #[arg(long)]
    setup: bool,

    /// Show current configuration
    #[arg(long)]
    config: bool,

    /// Change the AI model (can switch providers)
    #[arg(long)]
    model: bool,

    /// Manage API keys
    #[arg(long)]
    key: bool,

    /// Show GPG key setup instructions
    #[arg(long)]
    gpginfo: bool,

    /// Create a git tag after committing (e.g., --tag v1.0.0)
    #[arg(long, value_name = "VERSION")]
    tag: Option<String>,

    /// Treat the current directory as a folder of repos: stage everything in
    /// each one, generate all commit messages in parallel, then review them
    /// one by one as they arrive
    #[arg(long)]
    root: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Explain in plain English what's going on in this repo: what you're in
    /// the middle of, what could bite you, and what to do next
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --config flag
    if cli.config {
        return config::show_config();
    }

    // Handle --model flag
    if cli.model {
        return setup::change_model();
    }

    // Handle --key flag
    if cli.key {
        return setup::manage_api_key();
    }

    // Handle --gpginfo flag
    if cli.gpginfo {
        ui::print_gpg_info();
        return Ok(());
    }

    // Handle --setup flag or first run
    if cli.setup || !config::config_exists() {
        setup::run_setup()?;
        if cli.setup {
            return Ok(());
        }
    }

    if let Some(Command::Status) = cli.command {
        return status::run().await;
    }

    if cli.root {
        return run_root_flow(cli.tag).await;
    }

    // Main commit flow
    run_commit_flow(cli.tag).await
}

/// Everything the review loop needs about one repo's staged change. Built in
/// the repo's directory, then carried along so the loop can `chdir` back.
struct Staged {
    changes: Vec<git::StagedChange>,
    diff: String,
    /// Survives diff trimming, so the model always knows the full scope
    summary: String,
    /// Paths the model is allowed to name when asked to show a diff
    paths: Vec<String>,
}

/// Read the staged change in the current directory. `None` when nothing is staged.
fn read_staged() -> Result<Option<Staged>> {
    let changes = git::get_staged_changes()?;
    if changes.is_empty() {
        return Ok(None);
    }
    let diff = git::get_staged_diff()?;
    if diff.is_empty() {
        return Ok(None);
    }
    let summary = git::change_summary(&changes);
    let paths = changes.iter().map(|c| c.path.clone()).collect();
    Ok(Some(Staged { changes, diff, summary, paths }))
}

async fn run_commit_flow(tag: Option<String>) -> Result<()> {
    // Display header
    ui::print_header();

    // Load config
    let cfg = config::load_config()?;

    let Some(staged) = read_staged()? else {
        ui::print_warning("No staged changes found. Stage some changes with `git add` first.");
        return Ok(());
    };

    // Display staged changes summary
    ui::print_staged_changes(&staged.changes);

    // Generate commit message with AI
    let spinner = ui::create_spinner("Generating commit message...");
    let first = ai::generate_commit(&cfg, &staged.summary, &staged.diff, &[]).await;
    spinner.finish_and_clear();

    review_and_commit(&cfg, &staged, first?, tag.as_deref()).await?;
    Ok(())
}

/// What happened to one repo during `--root`
enum Outcome {
    Committed,
    Skipped,
    Failed(String),
}

/// One child repo with something to commit
struct RepoJob {
    name: String,
    path: PathBuf,
    staged: Staged,
}

/// `lgit --root`: every repo one level below the current directory gets
/// `git add -A`, its commit message is generated in parallel with the others,
/// and the developer reviews each one as soon as its message is ready.
async fn run_root_flow(tag: Option<String>) -> Result<()> {
    ui::print_header();
    let cfg = config::load_config()?;
    let root = std::env::current_dir().context("Could not read the current directory")?;

    let repos = child_repos(&root)?;
    if repos.is_empty() {
        ui::print_warning(&format!("No git repositories found in {}", root.display()));
        return Ok(());
    }

    // Stage and read every repo up front. This part is sequential and fast;
    // the slow part (the model) is what runs in parallel below.
    let mut jobs: Vec<RepoJob> = Vec::new();
    let mut clean: Vec<String> = Vec::new();
    let mut outcomes: Vec<(String, Outcome)> = Vec::new();
    for path in repos {
        let name = repo_name(&path);
        std::env::set_current_dir(&path)
            .with_context(|| format!("Could not enter {}", path.display()))?;
        if let Err(e) = git::stage_all() {
            ui::print_warning(&format!("{name}: {e}"));
            outcomes.push((name, Outcome::Failed(e.to_string())));
            continue;
        }
        match read_staged() {
            Ok(Some(staged)) => jobs.push(RepoJob { name, path, staged }),
            Ok(None) => clean.push(name),
            Err(e) => {
                ui::print_warning(&format!("{name}: {e}"));
                outcomes.push((name, Outcome::Failed(e.to_string())));
            }
        }
    }
    std::env::set_current_dir(&root)?;

    if !clean.is_empty() {
        ui::print_info(&format!("Nothing to commit in: {}", clean.join(", ")));
    }
    if jobs.is_empty() {
        ui::print_root_summary(&[], &outcomes);
        return Ok(());
    }
    ui::print_info(&format!(
        "Generating {} commit message(s) in parallel...",
        jobs.len()
    ));

    // One task per repo. Each sends back its index so the review loop can
    // take repos in whatever order the model finishes them.
    let (tx, mut rx) = mpsc::unbounded_channel::<(usize, Result<String>)>();
    for (idx, job) in jobs.iter().enumerate() {
        let tx = tx.clone();
        let cfg = cfg.clone();
        let summary = job.staged.summary.clone();
        let diff = job.staged.diff.clone();
        tokio::spawn(async move {
            let result = ai::generate_commit(&cfg, &summary, &diff, &[]).await;
            let _ = tx.send((idx, result));
        });
    }
    drop(tx);

    let total = jobs.len();
    let mut done = 0;
    while done < total {
        let spinner = ui::create_spinner(&format!(
            "Waiting for the next message ({} of {} left)...",
            total - done,
            total
        ));
        let next = rx.recv().await;
        spinner.finish_and_clear();
        let Some((idx, result)) = next else { break };
        done += 1;

        let job = &jobs[idx];
        ui::print_repo_heading(&job.name, done, total);

        let message = match result {
            Ok(message) => message,
            Err(e) => {
                ui::print_warning(&format!("Could not generate a message: {e}"));
                outcomes.push((job.name.clone(), Outcome::Failed(e.to_string())));
                continue;
            }
        };

        std::env::set_current_dir(&job.path)
            .with_context(|| format!("Could not enter {}", job.path.display()))?;
        ui::print_staged_changes(&job.staged.changes);

        let outcome = match review_and_commit(&cfg, &job.staged, message, tag.as_deref()).await {
            Ok(true) => Outcome::Committed,
            Ok(false) => Outcome::Skipped,
            Err(e) => {
                ui::print_warning(&format!("{e}"));
                Outcome::Failed(e.to_string())
            }
        };
        outcomes.push((job.name.clone(), outcome));
        std::env::set_current_dir(&root)?;
    }

    ui::print_root_summary(&clean, &outcomes);
    Ok(())
}

/// Direct children of `root` that are git repositories, sorted by name.
/// A `.git` file (not a directory) is a worktree, which counts too.
fn child_repos(root: &Path) -> Result<Vec<PathBuf>> {
    let mut repos: Vec<PathBuf> = std::fs::read_dir(root)
        .with_context(|| format!("Could not list {}", root.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir() && path.join(".git").exists())
        .collect();
    repos.sort();
    Ok(repos)
}

fn repo_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// The interactive loop over one proposed message: accept, edit, ask,
/// regenerate, or cancel. Runs against the repo in the current directory.
/// Returns `true` when a commit was made and `false` when the developer
/// cancelled.
async fn review_and_commit(
    cfg: &config::Config,
    staged: &Staged,
    first_message: String,
    tag: Option<&str>,
) -> Result<bool> {
    let mut commit_msg = first_message;

    // Everything said in follow-up turns, so a later regenerate honours it
    let mut history: Vec<String> = Vec::new();

    loop {
        ui::print_commit_message(&commit_msg);

        match ui::prompt_action()? {
            ui::UserAction::Accept => {
                // Get available GPG keys and prompt for signing choice
                let gpg_keys = git::list_gpg_keys()?;

                let committed = if gpg_keys.is_empty() {
                    // No GPG keys - offer unsigned commit or show help
                    ui::print_warning("No GPG keys found. Run `lgit --gpginfo` for setup instructions.");
                    if ui::prompt_unsigned_commit()? {
                        git::commit_unsigned(&commit_msg)?;
                        ui::print_success("Committed successfully (unsigned)!");
                        true
                    } else {
                        ui::print_info("Cancelled.");
                        false
                    }
                } else {
                    // GPG keys available - let user choose
                    match ui::prompt_signing_choice(&gpg_keys)? {
                        ui::SigningChoice::Signed(key) => {
                            git::commit_signed(&commit_msg, &key.key_id)?;
                            ui::print_success("Committed successfully (signed)!");
                            true
                        }
                        ui::SigningChoice::Unsigned => {
                            git::commit_unsigned(&commit_msg)?;
                            ui::print_success("Committed successfully (unsigned)!");
                            true
                        }
                    }
                };

                if !committed {
                    return Ok(false);
                }

                // Create tag if specified
                let tag_created = if let Some(tag_name) = tag {
                    match git::create_tag(tag_name) {
                        Ok(()) => {
                            ui::print_success(&format!("Created tag: {tag_name}"));
                            true
                        }
                        Err(e) => {
                            ui::print_warning(&format!("Failed to create tag: {e}"));
                            false
                        }
                    }
                } else {
                    false
                };

                // Push if configured
                if cfg.git.auto_push {
                    let push_msg = if tag_created {
                        "Pushing to remote (with tags)..."
                    } else {
                        "Pushing to remote..."
                    };
                    ui::print_info(push_msg);

                    let push_result = if tag_created {
                        git::push_with_tags()
                    } else {
                        git::push()
                    };

                    match push_result {
                        Ok(true) => ui::print_success("Pushed successfully!"),
                        Ok(false) => {
                            // Push rejected due to remote changes, pull and retry
                            ui::print_info("Remote has new changes, pulling...");
                            match git::pull() {
                                Ok(()) => {
                                    ui::print_info("Retrying push...");
                                    let retry_result = if tag_created {
                                        git::push_with_tags()
                                    } else {
                                        git::push()
                                    };
                                    match retry_result {
                                        Ok(true) => ui::print_success("Pushed successfully!"),
                                        Ok(false) => ui::print_warning("Push still rejected after pull. Please resolve manually."),
                                        Err(e) => ui::print_warning(&format!("Push failed: {e}")),
                                    }
                                }
                                Err(e) => ui::print_warning(&format!("Pull failed: {e}")),
                            }
                        }
                        Err(e) => ui::print_warning(&format!("Push failed: {e}")),
                    }
                }

                // Show PR link if configured
                if cfg.git.pr_link {
                    if let Some(url) = git::get_pr_url()? {
                        ui::print_pr_link(&url);
                    }
                }

                return Ok(true);
            }
            ui::UserAction::Edit => {
                commit_msg = ui::edit_message(&commit_msg)?;
            }
            ui::UserAction::Ask => {
                let question = ui::prompt_question()?;
                if question.is_empty() {
                    continue;
                }
                history.push(format!("Developer: {question}"));

                let spinner = ui::create_spinner("Looking at the diff...");
                let reply =
                    ai::follow_up(cfg, &staged.diff, &commit_msg, &staged.paths, &history).await;
                spinner.finish_and_clear();

                match reply? {
                    ai::FollowUp::Message(message) => {
                        history.push("You: rewrote the commit message as asked.".to_string());
                        commit_msg = message;
                    }
                    ai::FollowUp::Show { files, note } => {
                        for path in &files {
                            match git::get_file_diff(path) {
                                Ok(file_diff) => ui::print_file_diff(path, &file_diff),
                                Err(e) => {
                                    ui::print_warning(&format!("Could not diff {path}: {e}"))
                                }
                            }
                        }
                        if !note.is_empty() {
                            ui::print_note(&note);
                        }
                        history.push(format!("You: {note}"));
                    }
                }
            }
            ui::UserAction::Regenerate => {
                let spinner = ui::create_spinner("Generating commit message...");
                let result =
                    ai::generate_commit(cfg, &staged.summary, &staged.diff, &history).await;
                spinner.finish_and_clear();
                commit_msg = result?;
            }
            ui::UserAction::Cancel => {
                ui::print_info("Cancelled.");
                return Ok(false);
            }
        }
    }
}
