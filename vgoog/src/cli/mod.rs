pub mod apps_script;
pub mod args;
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
    /// Sign in without the wizard — for scripts, agents and fresh machines
    Login {
        /// Account name to create or replace
        #[arg(long, default_value = "default")]
        account: String,
        /// OAuth client id (Desktop app). Omit to read VGOOG_CLIENT_ID from the environment
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth client secret. Omit to read VGOOG_CLIENT_SECRET from the environment
        #[arg(long)]
        client_secret: Option<String>,
        /// Path to a service account JSON key — switches this account to domain-wide delegation
        #[arg(long)]
        service_account: Option<String>,
        /// Workspace user to impersonate. Required with --service-account
        #[arg(long)]
        subject: Option<String>,
    },
    /// What is configured, what it can reach, and what is wrong with it
    Doctor,
    /// Which credential kind to prefer: auto, service_account, or oauth
    Strategy {
        /// Omit to print the current setting
        kind: Option<String>,
    },
    /// Print the scopes to authorise, for the admin console or an OAuth client
    Scopes {
        /// Include Workspace-admin scopes that only a delegated service account can use
        #[arg(long)]
        all: bool,
    },
}
