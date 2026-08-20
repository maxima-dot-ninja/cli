pub mod apps_script;
pub mod calendar;
pub mod docs;
pub mod drive;
pub mod exec;
pub mod forms;
pub mod gmail;
pub mod people;
pub mod sheets;
pub mod slides;
pub mod tasks;

use clap::{Parser, Subcommand};

#[derive(Parser)]
// `version` is load-bearing: vaulty checks the installed binary against what a skill manifest
// claims, and a CLI that cannot report its version cannot be checked for drift.
#[command(name = "vgoog", version, about = "Google Workspace CLI & TUI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand)]
pub enum CliCommand {
    /// Execute a service action and return JSON
    Exec {
        /// Service: gmail, calendar, drive, sheets, docs, slides, forms, tasks, contacts, apps_script
        service: String,
        /// Action name (snake_case)
        action: String,
        /// JSON arguments (optional)
        args: Option<String>,
        /// Account name to use (overrides active_account)
        #[arg(long)]
        account: Option<String>,
    },
    /// List all available services and actions
    List,
    /// Check auth status
    Status,
}
