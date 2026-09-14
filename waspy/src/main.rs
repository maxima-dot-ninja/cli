//! waspy — WhatsApp Spy. Reads your WhatsApp straight out of the desktop app's own database, and
//! never writes to it. Bare `waspy` opens a menu; every entry is also a command, with `--json` for
//! scripts and agents.

mod ask;
mod commands;
mod db;
mod menu;
mod render;
mod store;
mod sync;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "waspy", version, about = "WhatsApp Spy: read your WhatsApp from the terminal. Read-only.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Print JSON instead of text, for scripts and agents
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Recent chats, newest first
    Chats {
        /// How many chats to list
        #[arg(short = 'n', default_value_t = 30)]
        count: usize,
    },
    /// Chats with unread messages, and those messages
    Unread,
    /// A chat's recent messages. Name the chat by any part of its name, or by its id
    Read {
        chat: Vec<String>,
        /// How many messages to show
        #[arg(short = 'n', default_value_t = 50)]
        count: usize,
        /// Only messages newer than this: 30m, 6h, 2d or 1w
        #[arg(long)]
        since: Option<String>,
    },
    /// Messages containing every word you give, across all chats, newest first
    Search {
        query: Vec<String>,
        /// How many messages to return
        #[arg(short = 'n', default_value_t = 20)]
        count: usize,
        /// Only search this chat
        #[arg(long)]
        chat: Option<String>,
    },
    /// A question in plain words, answered by Claude on your Claude subscription: "what did jen last say to me"
    Ask {
        question: Vec<String>,
        /// The Claude model: haiku, sonnet or opus
        #[arg(long, default_value = ask::MODEL)]
        model: String,
    },
    /// Copy WhatsApp's database into waspy's folder now
    Sync {
        /// Say nothing unless it fails (for the launchd job)
        #[arg(long)]
        quiet: bool,
    },
    /// Where waspy reads from, how fresh that is, and whether the sync job runs
    Status,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // Like pocket: a command waspy does not know opens the menu instead of a wall of usage.
        Err(error) if error.kind() == ErrorKind::InvalidSubcommand && menu::interactive() => {
            Cli { command: None, json: false }
        }
        Err(error) => error.exit(),
    };
    let json = cli.json;
    let outcome = match cli.command {
        None => menu::run(),
        Some(Command::Chats { count }) => commands::chats(count, json),
        Some(Command::Unread) => commands::unread(json),
        Some(Command::Read { chat, count, since }) => commands::read(&chat.join(" "), count, since.as_deref(), json),
        Some(Command::Search { query, count, chat }) => commands::search(&query.join(" "), count, chat.as_deref(), json),
        Some(Command::Ask { question, model }) => ask::run(&question.join(" "), &model, json),
        Some(Command::Sync { quiet }) => sync::run(quiet),
        Some(Command::Status) => sync::status(),
    };
    let Err(error) = outcome else { return };
    render::failure(&error, json);
    std::process::exit(1);
}
