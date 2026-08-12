use crate::git::{GpgKey, StagedChange};
use anyhow::{Context, Result};
use console::{style, Term};
use dialoguer::{Editor, Select};
use indicatif::{ProgressBar, ProgressStyle};

/// User action choices
pub enum UserAction {
    Accept,
    Edit,
    Regenerate,
    Cancel,
}

/// Signing choice
pub enum SigningChoice {
    Signed(crate::git::GpgKey),
    Unsigned,
}

/// Print the main header
pub fn print_header() {
    println!();
    println!(
        "{}",
        style("┌─────────────────────────────────────────────┐").cyan()
    );
    println!(
        "{}",
        style("│  lgit — AI-powered commits                  │").cyan()
    );
    println!(
        "{}",
        style("└─────────────────────────────────────────────┘").cyan()
    );
    println!();
}

/// Print the list of staged changes
pub fn print_staged_changes(changes: &[StagedChange]) {
    println!(
        "{} Staged changes ({} files):",
        style("📁").bold(),
        changes.len()
    );
    println!();

    for change in changes {
        let status_str = match change.status {
            crate::git::ChangeStatus::Added => style("added").green(),
            crate::git::ChangeStatus::Modified => style("modified").yellow(),
            crate::git::ChangeStatus::Deleted => style("deleted").red(),
            crate::git::ChangeStatus::Renamed => style("renamed").blue(),
        };

        let stats = if change.additions > 0 || change.deletions > 0 {
            format!(
                "{}  {}",
                style(format!("+{}", change.additions)).green(),
                style(format!("-{}", change.deletions)).red()
            )
        } else {
            String::new()
        };

        println!(
            "  {:<10} {:<40} {}",
            status_str,
            style(&change.path).dim(),
            stats
        );
    }

    println!();
    print_separator();
}

/// Print a separator line
pub fn print_separator() {
    println!("{}", style("━".repeat(45)).dim());
    println!();
}

/// Print the suggested commit message
pub fn print_commit_message(message: &str) {
    println!("{} Suggested commit:", style("📝").bold());
    println!();

    for line in message.lines() {
        println!("  {}", style(line).white());
    }

    println!();
    print_separator();
}

/// Prompt the user for their action choice
pub fn prompt_action() -> Result<UserAction> {
    let items = vec![
        "✓ Accept and commit",
        "✎ Edit message",
        "↻ Regenerate",
        "✕ Cancel",
    ];

    let selection = Select::new()
        .with_prompt("What would you like to do?")
        .items(&items)
        .default(0)
        .interact()
        .context("Failed to get user selection")?;

    Ok(match selection {
        0 => UserAction::Accept,
        1 => UserAction::Edit,
        2 => UserAction::Regenerate,
        _ => UserAction::Cancel,
    })
}

/// Open an editor for the user to modify the commit message
pub fn edit_message(current: &str) -> Result<String> {
    let edited = Editor::new()
        .extension(".txt")
        .edit(current)
        .context("Failed to open editor")?
        .unwrap_or_else(|| current.to_string());

    Ok(edited.trim().to_string())
}

/// Prompt user to select signing option (GPG key or unsigned)
pub fn prompt_signing_choice(keys: &[GpgKey]) -> Result<SigningChoice> {
    let mut items: Vec<String> = keys
        .iter()
        .map(|k| format!("🔐 {} ({})", k.user_id, k.key_id))
        .collect();
    items.push("📝 Commit without signing".to_string());

    let selection = Select::new()
        .with_prompt("Select signing option")
        .items(&items)
        .default(0)
        .interact()
        .context("Failed to get signing selection")?;

    if selection < keys.len() {
        Ok(SigningChoice::Signed(keys[selection].clone()))
    } else {
        Ok(SigningChoice::Unsigned)
    }
}

/// Prompt for unsigned commit when no GPG keys available
pub fn prompt_unsigned_commit() -> Result<bool> {
    let items = vec![
        "📝 Commit without signing",
        "✕ Cancel",
    ];

    let selection = Select::new()
        .with_prompt("No GPG keys found. What would you like to do?")
        .items(&items)
        .default(0)
        .interact()
        .context("Failed to get selection")?;

    Ok(selection == 0)
}

/// Display GPG setup instructions
pub fn print_gpg_info() {
    println!();
    println!(
        "{}",
        style("┌─────────────────────────────────────────────┐").cyan()
    );
    println!(
        "{}",
        style("│  GPG Key Setup Guide                        │").cyan()
    );
    println!(
        "{}",
        style("└─────────────────────────────────────────────┘").cyan()
    );
    println!();

    println!("{}", style("1. Check for existing keys").bold().yellow());
    println!("   gpg --list-secret-keys --keyid-format LONG");
    println!();

    println!("{}", style("2. Generate a new GPG key").bold().yellow());
    println!("   gpg --full-generate-key");
    println!();
    println!("   Recommended settings:");
    println!("   • Key type: RSA and RSA (default)");
    println!("   • Key size: 4096 bits");
    println!("   • Expiration: 1 year (or your preference)");
    println!("   • Use your Git email address");
    println!();

    println!("{}", style("3. Get your key ID").bold().yellow());
    println!("   gpg --list-secret-keys --keyid-format LONG");
    println!();
    println!("   Look for the line starting with 'sec', e.g.:");
    println!("   sec   rsa4096/{} 2024-01-01 [SC]", style("ABCD1234EFGH5678").green());
    println!("                 ^^^^^^^^^^^^^^^^");
    println!("                 This is your key ID");
    println!();

    println!("{}", style("4. Configure Git to use your key").bold().yellow());
    println!("   git config --global user.signingkey YOUR_KEY_ID");
    println!();

    println!("{}", style("5. Add to GitHub/GitLab").bold().yellow());
    println!("   Export your public key:");
    println!("   gpg --armor --export YOUR_KEY_ID");
    println!();
    println!("   Then add it to:");
    println!("   • GitHub: Settings → SSH and GPG keys → New GPG key");
    println!("   • GitLab: Preferences → GPG Keys");
    println!();

    println!("{}", style("Troubleshooting").bold().yellow());
    println!("   If signing fails, try:");
    println!("   export GPG_TTY=$(tty)");
    println!("   Add this to your ~/.bashrc or ~/.zshrc");
    println!();

    println!(
        "{}",
        style("For detailed instructions, see: docs/GPG_SETUP.md").dim()
    );
    println!();
}

/// Create a spinner for long-running operations
pub fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

/// Print a success message
pub fn print_success(message: &str) {
    println!("{} {}", style("✓").green().bold(), message);
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("{} {}", style("ℹ").cyan(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
    println!("{} {}", style("⚠").yellow(), message);
}

/// Print the PR creation link
pub fn print_pr_link(url: &str) {
    println!();
    println!("{} Create a pull request:", style("🔗").bold());
    println!("  {}", style(url).cyan().underlined());
    println!();
}

/// Clear the terminal screen
#[allow(dead_code)]
pub fn clear_screen() {
    let term = Term::stdout();
    let _ = term.clear_screen();
}
