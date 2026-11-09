//! What `hku` does when you run it with nothing to go on.
//!
//! Every path here is also reachable non-interactively — the wizard only picks
//! the arguments, then hands them to the same code a flag would have reached.

use crate::client::{segment, Client};
use crate::config::{self, Config};
use crate::view;
use anyhow::{bail, Result};
use console::style;
use dialoguer::{Confirm, FuzzySelect, Input, Password, Select};
use serde_json::Value;

type View = fn(&Client, &str) -> Result<()>;

/// What you can look at on one app, in the order the menu shows them.
const VIEWS: &[(&str, View)] = &[
    ("Details", |client, app| crate::app_show(client, app, false)),
    ("Dynos", |client, app| crate::dynos(client, app, false)),
    ("Formation", |client, app| crate::formation(client, app, false)),
    ("Config vars, masked", |client, app| crate::config_vars(client, app, false, false)),
    ("Config vars, with values", |client, app| crate::config_vars(client, app, true, false)),
    ("Releases", |client, app| crate::releases(client, app, 20, false)),
    ("Add-ons", |client, app| crate::addons(client, app, false)),
    ("Domains", |client, app| crate::domains(client, app, false)),
    ("Recent logs", |client, app| crate::logs(client, app, 100, None, None, None, false)),
];

pub fn run(config: &Config) -> Result<()> {
    view::header();
    if config.api_key.is_empty() {
        return setup(config).map(|_| ());
    }
    let client = Client::new(config)?;

    let actions = ["List apps", "Look at one app", "Call the API directly", "Config", "Quit"];
    loop {
        let pick =
            Select::new().with_prompt("What do you want to do?").items(&actions).default(0).interact()?;
        match pick {
            0 => crate::apps(&client, &config.team, false)?,
            1 => one_app(&client, config)?,
            2 => raw_call(&client)?,
            3 => crate::show_config(config)?,
            _ => return Ok(()),
        }
    }
}

fn one_app(client: &Client, config: &Config) -> Result<()> {
    let names: Vec<String> = crate::list_apps(client, &config.team)?
        .iter()
        .filter_map(|app| app.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    if names.is_empty() {
        bail!("The token reaches no apps");
    }
    let pick = FuzzySelect::new().with_prompt("Which app?").items(&names).default(0).interact()?;
    let app = &names[pick];

    let labels: Vec<&str> = VIEWS.iter().map(|(label, _)| *label).chain(["Back"]).collect();
    loop {
        let pick = Select::new().with_prompt(app.as_str()).items(&labels).default(0).interact()?;
        let Some((_, show)) = VIEWS.get(pick) else { return Ok(()) };
        show(client, app)?;
    }
}

fn raw_call(client: &Client) -> Result<()> {
    let methods = ["GET", "POST", "PATCH", "PUT", "DELETE"];
    let method = methods[Select::new().with_prompt("Method").items(&methods).default(0).interact()?];
    let path: String = Input::new().with_prompt("Path, such as apps/my-app/dynos").interact_text()?;
    let body = match method {
        "GET" => String::new(),
        _ => Input::new().with_prompt("JSON body (blank for none)").allow_empty(true).interact_text()?,
    };
    crate::call(client, method, &path, Some(body).filter(|b| !b.trim().is_empty()), false, false)
}

/// First run: take the token and a team, prove they work, then write them down.
pub fn setup(config: &Config) -> Result<Option<Config>> {
    println!("{}\n", config::missing_help());
    if !Confirm::new().with_prompt("Set it up now?").default(true).interact()? {
        return Ok(None);
    }
    let api_key: String = Password::new().with_prompt("Heroku token").interact()?;
    let team = Input::new().with_prompt("Default team (blank for every app)").allow_empty(true);
    let team: String = match config.team.is_empty() {
        true => team.interact_text()?,
        false => team.default(config.team.clone()).interact_text()?,
    };

    let candidate = Config { api_key: api_key.trim().to_string(), team: team.trim().to_string() };
    let client = Client::new(&candidate)?;
    let account = client.get("/account")?;
    let email = account.get("email").and_then(Value::as_str).unwrap_or("that account");
    println!("\n  {} Connected as {}", style("✓").green(), style(email).bold());
    if !candidate.team.is_empty() {
        let found = client.get(&format!("/teams/{}", segment(&candidate.team)))?;
        let name = found.get("name").and_then(Value::as_str).unwrap_or(&candidate.team);
        println!("  {} Team {}", style("✓").green(), style(name).bold());
    }

    config::save(&candidate)?;
    println!("  Saved to {}\n", config::config_path()?.display());
    Ok(Some(candidate))
}
