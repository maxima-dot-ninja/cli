//! `lgit pr`: pull requests on GitHub, for a program rather than a person. Every
//! answer is one JSON document on stdout and nothing goes to stderr, so a caller reads
//! stdout whether the command worked or not. A refusal exits 1 with
//! `{ "error", "reason" }` — and for a merge, `"merged": false` as well.
//!
//! These never run the setup wizard and never read lgit's config: Vaulty calls them
//! from launchd, where there is nobody at the keyboard and no model is needed.

mod diff;
mod gh;
mod list;
mod merge;

use clap::Subcommand;
use gh::{repo_of, Failure, Gh};
use serde_json::{json, Value};
use std::time::Duration;

/// The whole command's budget unless `--timeout-ms` says otherwise. Vaulty's GitHub
/// calls get 20 seconds, so this matches it.
const TIMEOUT_MS: u64 = 20_000;

#[derive(Subcommand)]
pub enum PrCommand {
    /// The open pull requests on each repo, as JSON
    List {
        /// Repos as owner/name or GitHub urls
        #[arg(required = true)]
        repos: Vec<String>,
        /// Also fetch every pull request's files and their diffs
        #[arg(long)]
        diffs: bool,
        #[arg(long, default_value_t = TIMEOUT_MS)]
        timeout_ms: u64,
    },
    /// One pull request's files and their diffs, at its head commit, as JSON
    Diff {
        /// owner/name or a GitHub url
        repo: String,
        number: u64,
        #[arg(long, default_value_t = TIMEOUT_MS)]
        timeout_ms: u64,
    },
    /// Merge one pull request, or say why not, as JSON
    Merge {
        /// owner/name or a GitHub url
        repo: String,
        number: u64,
        /// The head commit you reviewed. The merge is refused if the branch has moved since.
        #[arg(long)]
        sha: Option<String>,
        /// squash, merge or rebase. Squash by default when the repo allows it.
        #[arg(long, value_parser = ["squash", "merge", "rebase"])]
        method: Option<String>,
        #[arg(long, default_value_t = TIMEOUT_MS)]
        timeout_ms: u64,
    },
}

/// Runs one and prints its answer. Answers the exit code: 0 when it did what was
/// asked, 1 when it refused or failed.
pub async fn run(command: &PrCommand) -> i32 {
    let answer = answer(command).await;
    let (code, out) = match answer {
        Ok(value) => (0, value),
        Err(failure) => (1, refusal(command, failure)),
    };
    println!("{out}");
    code
}

async fn answer(command: &PrCommand) -> Result<Value, Failure> {
    match command {
        PrCommand::List { repos, diffs, timeout_ms } => list::run(&gh(*timeout_ms)?, repos, *diffs).await,
        PrCommand::Diff { repo, number, timeout_ms } => {
            let (owner, name) = parse_repo(repo)?;
            diff::fetch(&gh(*timeout_ms)?, &owner, &name, *number).await
        }
        PrCommand::Merge { repo, number, sha, method, timeout_ms } => {
            let (owner, name) = parse_repo(repo)?;
            let ask = merge::Ask { owner: &owner, name: &name, number: *number, sha: sha.as_deref(), method: method.as_deref() };
            merge::run(&gh(*timeout_ms)?, ask).await
        }
    }
}

fn gh(timeout_ms: u64) -> Result<Gh, Failure> {
    Gh::find(Duration::from_millis(timeout_ms))
}

fn parse_repo(text: &str) -> Result<(String, String), Failure> {
    repo_of(text).ok_or_else(|| Failure::new("bad_repo", format!("{text} is not owner/name or a GitHub url")))
}

/// A merge's refusal also says `"merged": false`, so a caller checks one field whatever
/// came back.
fn refusal(command: &PrCommand, failure: Failure) -> Value {
    let mut out = json!({ "error": failure.error, "reason": failure.reason });
    if let PrCommand::Merge { repo, number, .. } = command {
        out["merged"] = json!(false);
        out["repo"] = json!(repo_of(repo).map(|(owner, name)| format!("{owner}/{name}")).unwrap_or_else(|| repo.clone()));
        out["number"] = json!(number);
    }
    out
}
