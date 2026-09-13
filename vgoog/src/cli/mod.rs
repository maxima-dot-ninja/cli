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

use crate::tier::Tier;
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
        /// Account to run this one call as. The active account stays as it is
        #[arg(long)]
        account: Option<String>,
    },
    /// List all available services and actions
    List,
    /// Check auth status, and print the account and its tier
    Status {
        /// Account to check. The active account stays as it is
        #[arg(long)]
        account: Option<String>,
    },
    /// List every configured account, with its tier and the address it acts as
    Accounts,
    /// Make an account the active one — the default for every later call
    Switch {
        /// Account name, as `vgoog accounts` lists it
        account: String,
    },
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
        /// Path to a service account JSON key. Omit it to reuse the key already saved on this machine
        #[arg(long)]
        service_account: Option<String>,
        /// Workspace user to impersonate — switches this account to domain-wide delegation
        #[arg(long)]
        subject: Option<String>,
        /// Whose mailbox this is: `user` never sends mail through vgoog, `ai` may
        #[arg(long, value_enum, default_value_t = Tier::User)]
        tier: Tier,
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
