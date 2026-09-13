//! What `phog` does when you run it with nothing to go on.
//!
//! Every path here is also reachable non-interactively — the wizard only picks
//! the arguments, then hands them to the same code a flag would have reached.

use crate::client::Client;
use crate::config::{self, Config};
use crate::{dashboards, view};
use anyhow::{bail, Result};
use console::style;
use dialoguer::{Confirm, FuzzySelect, Input, Password, Select};

pub fn run(config: &Config) -> Result<()> {
    view::header();
    if config.api_key.is_empty() || config.project_id.is_empty() {
        return setup(config).map(|_| ());
    }
    let client = Client::new(config)?;

    let actions = ["Run a HogQL query", "Look someone up", "Dashboards", "Journey counts", "Config", "Quit"];
    loop {
        let pick =
            Select::new().with_prompt("What do you want to do?").items(&actions).default(0).interact()?;
        match pick {
            0 => query(&client)?,
            1 => lookup(&client)?,
            2 => dashboards_menu(&client)?,
            3 => journey_counts(&client)?,
            4 => crate::show_config(config)?,
            _ => return Ok(()),
        }
    }
}

fn query(client: &Client) -> Result<()> {
    let hogql: String = Input::new().with_prompt("HogQL").interact_text()?;
    let result = client.query(&hogql)?;
    view::query_result(&result, false)
}

fn lookup(client: &Client) -> Result<()> {
    let kinds = ["by fingerprint", "by email", "by distinct id"];
    let kind = Select::new().with_prompt("Find them").items(&kinds).default(0).interact()?;
    let value: String = Input::new().with_prompt("Value").interact_text()?;
    match kind {
        0 => crate::events(client, Some(value), None, None, "-90d", 100, false),
        1 => crate::events(client, None, Some(value), None, "-90d", 100, false),
        _ => crate::events(client, None, None, Some(value), "-90d", 100, false),
    }
}

fn dashboards_menu(client: &Client) -> Result<()> {
    let actions =
        ["List", "Apply a file", "Preview what a file would change", "Export one to a file", "Back"];
    let pick = Select::new().with_prompt("Dashboards").items(&actions).default(0).interact()?;
    match pick {
        0 => view::dashboards(&dashboards::list(client)?, false),
        1 => {
            let file = pick_file()?;
            crate::apply(client, &file, false, true)
        }
        2 => {
            let file = pick_file()?;
            crate::apply(client, &file, true, true)
        }
        3 => {
            let all = dashboards::list(client)?;
            let names: Vec<String> = all
                .iter()
                .map(|d| {
                    format!(
                        "{}  {}",
                        d.get("id").and_then(|i| i.as_u64()).unwrap_or_default(),
                        d.get("name").and_then(|n| n.as_str()).unwrap_or_default()
                    )
                })
                .collect();
            if names.is_empty() {
                bail!("There are no dashboards to export");
            }
            let pick = FuzzySelect::new().with_prompt("Which one?").items(&names).default(0).interact()?;
            let id = all[pick].get("id").and_then(|i| i.as_u64()).unwrap_or_default();
            let out: String = Input::new()
                .with_prompt("Write to")
                .default(format!(
                    "dashboards/{}.yaml",
                    crate::spec::slugify(names[pick].split("  ").nth(1).unwrap_or("dashboard"))
                ))
                .interact_text()?;
            crate::export(client, &id.to_string(), Some(out))
        }
        _ => Ok(()),
    }
}

/// The same thing `phog journeys` does, with the arguments asked for one by one.
fn journey_counts(client: &Client) -> Result<()> {
    let months: usize = Input::new().with_prompt("How many months").default(12).interact_text()?;
    let paths: String = Input::new()
        .with_prompt("Route patterns for $pageview, space-separated (blank for none)")
        .allow_empty(true)
        .interact_text()?;
    let out: String =
        Input::new().with_prompt("Write to (blank prints it)").allow_empty(true).interact_text()?;
    let paths = paths.split_whitespace().map(str::to_string).collect();
    crate::journeys::run(client, months, paths, Some(out).filter(|o| !o.trim().is_empty()))
}

fn pick_file() -> Result<String> {
    let mut files: Vec<String> = std::fs::read_dir("dashboards")
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .map(|e| e.path().display().to_string())
                .filter(|p| p.ends_with(".yaml") || p.ends_with(".yml"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    if files.is_empty() {
        return Ok(Input::new().with_prompt("Dashboard file").interact_text()?);
    }
    files.push("Somewhere else…".to_string());
    let pick = FuzzySelect::new().with_prompt("Which file?").items(&files).default(0).interact()?;
    if pick == files.len() - 1 {
        return Ok(Input::new().with_prompt("Dashboard file").interact_text()?);
    }
    Ok(files[pick].clone())
}

/// First run: take the key and the project, prove they work, then write them down.
pub fn setup(config: &Config) -> Result<Option<Config>> {
    println!("{}\n", config::missing_help());
    if !Confirm::new().with_prompt("Set it up now?").default(true).interact()? {
        return Ok(None);
    }
    let host: String = Input::new().with_prompt("App host").default(config.host.clone()).interact_text()?;
    let api_key: String = Password::new().with_prompt("Personal API key (phx_…)").interact()?;
    let project_id: String =
        Input::new().with_prompt("Project id").default(config.project_id.clone()).interact_text()?;

    let candidate = Config {
        api_key: api_key.trim().to_string(),
        project_id: project_id.trim().to_string(),
        host: host.trim_end_matches('/').to_string(),
    };
    let client = Client::new(&candidate)?;
    let project = client.get(&client.project_path(""), &[])?;
    let name = project.get("name").and_then(|n| n.as_str()).unwrap_or("that project");
    println!("\n  {} Connected to {}", style("✓").green(), style(name).bold());

    config::save(&candidate)?;
    println!("  Saved to {}\n", config::config_path()?.display());
    Ok(Some(candidate))
}
