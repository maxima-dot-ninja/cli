//! Pocket AI recordings from the terminal: list them, export them to disk, and
//! search the exports in natural language through qmd.

mod api;
mod export;
mod format;
mod search;
mod select;

use anyhow::Result;
use api::Client;
use std::path::PathBuf;

/// One fixed home for exports, never the current directory — the search index
/// and any agent reading these files need a path that does not depend on where
/// pocket ran.
pub fn export_root() -> PathBuf {
    match std::env::var("POCKET_EXPORT_DIR") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => dirs::home_dir().unwrap_or_default().join("dev").join("pocket-exports"),
    }
}

/// Parsed by hand rather than with clap, so the command line reads the way it
/// always has: flags sit anywhere, `search` joins every word after it into one
/// query, and anything unknown opens the menu.
#[derive(Default)]
struct Cli {
    args: Vec<String>,
    search: String,
    count: String,
    json: bool,
    fast: bool,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Cli {
    let mut cli = Cli::default();
    while let Some(token) = argv.next() {
        match token.as_str() {
            "--json" => cli.json = true,
            "--fast" => cli.fast = true,
            "--search" => cli.search = argv.next().unwrap_or_default(),
            "-n" => cli.count = argv.next().unwrap_or_default(),
            _ => cli.args.push(token),
        }
    }
    cli
}

/// `--search` takes the next word; `search` takes every word after it.
fn query_of(cli: &Cli) -> String {
    if !cli.search.is_empty() {
        return cli.search.clone();
    }
    if cli.args.first().map(String::as_str) != Some("search") {
        return String::new();
    }
    cli.args[1..].join(" ")
}

fn search(cli: &Cli, query: &str) -> Result<()> {
    let limit = cli.count.parse().ok().filter(|&n: &usize| n > 0).unwrap_or(5);
    let results = search::search(query, limit, !cli.fast)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }
    println!("{}", format::search_results_text(&results));
    Ok(())
}

fn search_interactive(cli: &Cli) -> Result<()> {
    let query = select::prompt("\nWhat are you looking for?")?;
    if query.is_empty() {
        return Ok(());
    }
    search(cli, &query)
}

fn list(_: &Cli) -> Result<()> {
    let recs = Client::new()?.list_recordings();
    println!();
    for rec in &recs {
        println!("{}  {}", format::text(rec, "id"), format::recording_label(rec));
    }
    println!("\n{} recordings", recs.len());
    Ok(())
}

fn export_one(_: &Cli) -> Result<()> {
    let client = Client::new()?;
    let recs = client.list_recordings();
    if recs.is_empty() {
        println!("No recordings found.");
        return Ok(());
    }
    let labels: Vec<String> = recs.iter().map(format::recording_label).collect();
    let pick = select::select("\nWhich recording?", &labels)?;
    export::one(&client, format::text(&recs[pick], "id"));
    search::reindex()
}

fn export_all(_: &Cli) -> Result<()> {
    let client = Client::new()?;
    let recs = client.list_recordings();
    println!();
    let ids: Vec<&str> = recs.iter().map(|rec| format::text(rec, "id")).collect();
    export::all(&client, &ids);
    search::reindex()
}

fn reindex(_: &Cli) -> Result<()> {
    search::reindex()
}

type Action = fn(&Cli) -> Result<()>;

const MENU: [(&str, Action); 5] = [
    ("Search conversations", search_interactive),
    ("List recordings", list),
    ("Export one recording", export_one),
    ("Export all recordings", export_all),
    ("Rebuild search index", reindex),
];

fn main() -> Result<()> {
    let cli = parse(std::env::args().skip(1));
    let cmd = cli.args.first().map(String::as_str).unwrap_or("");
    let arg = cli.args.get(1).map(String::as_str).unwrap_or("");

    // Search needs no API key — it only reads what was already exported.
    let query = query_of(&cli);
    if !query.is_empty() {
        return search(&cli, &query);
    }
    if cmd == "index" {
        return search::reindex();
    }

    if api::app_key().is_empty() {
        println!("No API key found.\nSet POCKET_APP_KEY or put the key in ~/.config/pocket/key");
        return Ok(());
    }

    match (cmd, arg) {
        ("list", _) => list(&cli),
        ("export", "all") => export_all(&cli),
        ("export", "") => export_one(&cli),
        ("export", id) => {
            export::one(&Client::new()?, id);
            search::reindex()
        }
        _ => {
            let names: Vec<String> = MENU.iter().map(|(name, _)| name.to_string()).collect();
            let pick = select::select("Pocket", &names)?;
            MENU[pick].1(&cli)
        }
    }
}
