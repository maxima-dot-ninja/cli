mod ai;
mod config;
mod git;
mod providers;
mod setup;
mod ui;

use anyhow::Result;
use clap::Parser;

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

    // Main commit flow
    run_commit_flow(cli.tag).await
}

async fn run_commit_flow(tag: Option<String>) -> Result<()> {
    // Display header
    ui::print_header();

    // Load config
    let cfg = config::load_config()?;

    // Check for staged changes
    let staged = git::get_staged_changes()?;
    if staged.is_empty() {
        ui::print_warning("No staged changes found. Stage some changes with `git add` first.");
        return Ok(());
    }

    // Display staged changes summary
    ui::print_staged_changes(&staged);

    // Get the diff for AI
    let diff = git::get_staged_diff()?;
    if diff.is_empty() {
        ui::print_warning("No diff content found.");
        return Ok(());
    }

    // Generate commit message with AI
    let mut commit_msg = generate_commit_message(&cfg, &diff).await?;

    // Interactive loop for user action
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
                    break;
                }

                // Create tag if specified
                let tag_created = if let Some(ref tag_name) = tag {
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

                break;
            }
            ui::UserAction::Edit => {
                commit_msg = ui::edit_message(&commit_msg)?;
            }
            ui::UserAction::Regenerate => {
                commit_msg = generate_commit_message(&cfg, &diff).await?;
            }
            ui::UserAction::Cancel => {
                ui::print_info("Cancelled.");
                break;
            }
        }
    }

    Ok(())
}

async fn generate_commit_message(cfg: &config::Config, diff: &str) -> Result<String> {
    let spinner = ui::create_spinner("Generating commit message...");

    let result = ai::generate_commit(cfg, diff).await;

    spinner.finish_and_clear();

    result
}
