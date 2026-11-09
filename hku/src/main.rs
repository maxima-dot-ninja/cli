mod client;
mod config;
mod view;
mod wizard;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use client::{segment, Client};
use config::Config;
use console::style;
use reqwest::Method;
use serde_json::{json, Value};
use std::io::IsTerminal;
use view::Column;

#[derive(Parser)]
#[command(name = "hku")]
#[command(about = "Heroku from the terminal — apps, dynos, config vars, releases and logs", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Print raw JSON instead of a table
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// With no app, show where config lives. With one, show its config vars: hku config my-app
    Config {
        app: Option<String>,
        /// Print the values. They are masked otherwise, because they are live secrets.
        #[arg(long)]
        reveal: bool,
    },
    /// Take a token and a default team, check them against the API, and save them
    Setup,
    /// Every app in the saved team, or every app the token reaches: hku apps ; hku apps show my-app
    Apps {
        #[command(subcommand)]
        command: Option<AppsCommand>,
        /// The team to list instead of the saved one
        #[arg(long)]
        team: Option<String>,
    },
    /// The dynos running now, and the state of each
    Dynos { app: String },
    /// The process types, how many dynos each runs, and their size
    Formation { app: String },
    /// Releases, newest first
    Releases {
        app: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// The add-ons on an app, with the config vars each one sets
    Addons { app: String },
    /// The domains on an app, and the DNS target for each
    Domains { app: String },
    /// Recent log lines: hku logs my-app --lines 200 --dyno web.1
    Logs {
        app: String,
        /// How many past lines. Cedar keeps the newest 1,500; Fir keeps none.
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1500))]
        lines: u32,
        /// One dyno, such as web.1
        #[arg(long)]
        dyno: Option<String>,
        /// app or heroku
        #[arg(long)]
        source: Option<String>,
        /// One process type, such as web
        #[arg(long = "type")]
        process_type: Option<String>,
        /// Keep the stream open and print lines as they arrive
        #[arg(long)]
        tail: bool,
    },
    /// Call any endpoint directly: hku call GET apps/my-app/dynos ; hku call PATCH apps/my-app '{"maintenance":true}'
    Call {
        /// GET, POST, PATCH, PUT or DELETE
        method: String,
        /// Path under https://api.heroku.com/
        path: String,
        /// A JSON body, for anything that writes
        body: Option<String>,
        /// Skip the confirmation on anything that writes
        #[arg(long)]
        yes: bool,
        /// Print config var values. They are masked otherwise, as in `hku config <app>`.
        #[arg(long)]
        reveal: bool,
    },
}

#[derive(Subcommand)]
enum AppsCommand {
    /// One app: its generation, region, stack, team and URLs
    Show { app: String },
}

const APPS: &[Column] = &[
    ("NAME", "/name"),
    ("GENERATION", "/generation/name"),
    ("REGION", "/region/name"),
    ("STACK", "/stack/name"),
    ("TEAM", "/team/name"),
    ("OWNER", "/owner/email"),
    ("RELEASED", "/released_at"),
];

const APP: &[Column] = &[
    ("Name", "/name"),
    ("Id", "/id"),
    ("Generation", "/generation/name"),
    ("Region", "/region/name"),
    ("Stack", "/stack/name"),
    ("Team", "/team/name"),
    ("Owner", "/owner/email"),
    ("Web URL", "/web_url"),
    ("Git URL", "/git_url"),
    ("Maintenance", "/maintenance"),
    ("ACM", "/acm"),
    ("Released", "/released_at"),
    ("Created", "/created_at"),
    ("Slug bytes", "/slug_size"),
];

const DYNOS: &[Column] = &[
    ("NAME", "/name"),
    ("TYPE", "/type"),
    ("SIZE", "/size"),
    ("STATE", "/state"),
    ("RELEASE", "/release/version"),
    ("COMMAND", "/command"),
    ("SINCE", "/updated_at"),
];

const FORMATION: &[Column] =
    &[("TYPE", "/type"), ("QUANTITY", "/quantity"), ("SIZE", "/dyno_size/name"), ("COMMAND", "/command")];

const RELEASES: &[Column] = &[
    ("VERSION", "/version"),
    ("STATUS", "/status"),
    ("CURRENT", "/current"),
    ("BY", "/user/email"),
    ("CREATED", "/created_at"),
    ("DESCRIPTION", "/description"),
];

const ADDONS: &[Column] = &[
    ("NAME", "/name"),
    ("SERVICE", "/addon_service/name"),
    ("PLAN", "/plan/name"),
    ("STATE", "/state"),
    ("SETS", "/config_vars"),
];

const DOMAINS: &[Column] = &[
    ("HOSTNAME", "/hostname"),
    ("KIND", "/kind"),
    ("DNS TARGET", "/cname"),
    ("STATUS", "/status"),
    ("ACM", "/acm_status"),
];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load()?;
    let as_json = cli.json;

    let Some(command) = cli.command else { return wizard::run(&config) };
    match command {
        Command::Config { app: None, .. } => show_config(&config),
        Command::Setup => wizard::setup(&config).map(|_| ()),
        Command::Config { app: Some(app), reveal } => {
            config_vars(&Client::new(&config)?, &app, reveal, as_json)
        }
        Command::Apps { command: Some(AppsCommand::Show { app }), .. } => {
            app_show(&Client::new(&config)?, &app, as_json)
        }
        Command::Apps { command: None, team } => {
            apps(&Client::new(&config)?, team.as_deref().unwrap_or(&config.team), as_json)
        }
        Command::Dynos { app } => dynos(&Client::new(&config)?, &app, as_json),
        Command::Formation { app } => formation(&Client::new(&config)?, &app, as_json),
        Command::Releases { app, limit } => releases(&Client::new(&config)?, &app, limit, as_json),
        Command::Addons { app } => addons(&Client::new(&config)?, &app, as_json),
        Command::Domains { app } => domains(&Client::new(&config)?, &app, as_json),
        Command::Logs { app, lines, dyno, source, process_type, tail } => {
            logs(&Client::new(&config)?, &app, lines, dyno, source, process_type, tail)
        }
        Command::Call { method, path, body, yes, reveal } => {
            call(&Client::new(&config)?, &method, &path, body, yes, reveal)
        }
    }
}

pub fn show_config(config: &Config) -> Result<()> {
    println!();
    println!("  Config file : {}", config::config_path()?.display());
    println!(
        "  API key     : {}",
        if config.api_key.is_empty() {
            style("not set").red().to_string()
        } else {
            format!("set ({} chars)", config.api_key.len())
        }
    );
    println!(
        "  Team        : {}",
        if config.team.is_empty() {
            style("none — `hku apps` lists every app the token reaches").dim().to_string()
        } else {
            config.team.clone()
        }
    );
    println!();
    if config.api_key.is_empty() {
        println!("{}\n", config::missing_help());
    }
    Ok(())
}

fn app_path(app: &str, suffix: &str) -> String {
    format!("/apps/{}{suffix}", segment(app))
}

/// Every app in a team, or every app the token reaches when the team is empty, by name.
pub fn list_apps(client: &Client, team: &str) -> Result<Vec<Value>> {
    let path = match team.is_empty() {
        true => "/apps".to_string(),
        false => format!("/teams/{}/apps", segment(team)),
    };
    let mut apps = client.list(&path, None, usize::MAX)?;
    apps.sort_by_key(|app| app.get("name").and_then(Value::as_str).unwrap_or_default().to_string());
    Ok(apps)
}

pub fn apps(client: &Client, team: &str, as_json: bool) -> Result<()> {
    view::list(&list_apps(client, team)?, APPS, as_json)
}

pub fn app_show(client: &Client, app: &str, as_json: bool) -> Result<()> {
    view::fields(&client.get(&app_path(app, ""))?, APP, as_json)
}

pub fn dynos(client: &Client, app: &str, as_json: bool) -> Result<()> {
    let dynos = client.list(&app_path(app, "/dynos"), None, usize::MAX)?;
    view::list(&dynos, DYNOS, as_json)?;
    if as_json {
        return Ok(());
    }
    let crashed: Vec<&str> = dynos
        .iter()
        .filter(|dyno| dyno.get("state").and_then(Value::as_str) == Some("crashed"))
        .filter_map(|dyno| dyno.get("name").and_then(Value::as_str))
        .collect();
    if !crashed.is_empty() {
        println!("  {} {} crashed: {}\n", style("✗").red(), crashed.len(), style(crashed.join(", ")).red());
    }
    Ok(())
}

pub fn formation(client: &Client, app: &str, as_json: bool) -> Result<()> {
    view::list(&client.list(&app_path(app, "/formation"), None, usize::MAX)?, FORMATION, as_json)
}

pub fn releases(client: &Client, app: &str, limit: usize, as_json: bool) -> Result<()> {
    // Heroku lists releases oldest first, so ask for the newest end of the range.
    let range = format!("version ..; max={}, order=desc", limit.clamp(1, 1000));
    view::list(&client.list(&app_path(app, "/releases"), Some(range), limit)?, RELEASES, as_json)
}

pub fn addons(client: &Client, app: &str, as_json: bool) -> Result<()> {
    view::list(&client.list(&app_path(app, "/addons"), None, usize::MAX)?, ADDONS, as_json)
}

pub fn domains(client: &Client, app: &str, as_json: bool) -> Result<()> {
    view::list(&client.list(&app_path(app, "/domains"), None, usize::MAX)?, DOMAINS, as_json)
}

/// Config vars are live secrets, and whatever this prints can land in an agent's transcript, so
/// each value is masked down to its length unless `reveal` is set.
pub fn config_vars(client: &Client, app: &str, reveal: bool, as_json: bool) -> Result<()> {
    let vars = client.get(&app_path(app, "/config-vars"))?;
    let vars = if reveal { vars } else { masked(vars) };
    let Value::Object(vars) = vars else { bail!("Heroku sent config vars that are not an object") };
    if as_json {
        return view::json(&Value::Object(vars));
    }
    let rows: Vec<Vec<Value>> =
        vars.into_iter().map(|(name, value)| vec![Value::String(name), value]).collect();
    view::table(&["NAME".to_string(), "VALUE".to_string()], &rows, if reveal { 200 } else { 60 });
    if !reveal && !rows.is_empty() {
        view::note("Values are masked. `--reveal` prints them.");
        println!();
    }
    Ok(())
}

/// The paths whose answers hold config var values: an app's or a release's config vars, and an
/// add-on's config.
const SECRET_PATHS: [&str; 2] = ["/config-vars", "/config"];

fn mask(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(format!("•••• {} chars", text.chars().count())),
        other => other,
    }
}

/// Config vars come back as one object of name to value, and an add-on's config as a list of
/// `{name, value}`. Either way every value is masked down to its length.
fn masked(answer: Value) -> Value {
    match answer {
        Value::Object(vars) => {
            Value::Object(vars.into_iter().map(|(name, value)| (name, mask(value))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(masked_entry).collect()),
        other => other,
    }
}

fn masked_entry(mut entry: Value) -> Value {
    if let Some(value) = entry.get_mut("value") {
        *value = mask(value.take());
    }
    entry
}

pub fn logs(
    client: &Client,
    app: &str,
    lines: u32,
    dyno: Option<String>,
    source: Option<String>,
    process_type: Option<String>,
    tail: bool,
) -> Result<()> {
    let mut options = json!({ "lines": lines, "tail": tail });
    for (key, value) in [("dyno_name", dyno), ("source", source), ("type", process_type)] {
        if let Some(value) = value {
            options[key] = Value::String(value);
        }
    }
    let count = client.logs(&app_path(app, "/log-sessions"), &options, tail, |line| println!("{line}"))?;
    if count > 0 || tail {
        return Ok(());
    }
    // Fir keeps no history, so an empty answer there means "nothing to replay", not "nothing
    // happened". Asking costs one more call, made only when the answer was empty.
    let generation = client
        .get(&app_path(app, ""))?
        .pointer("/generation/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    if generation.as_deref() == Some("fir") {
        bail!(
            "{app} runs on Fir, which keeps no log history. `hku logs {app} --tail` prints lines as they \
             arrive; anything older needs a telemetry drain."
        );
    }
    view::note("No log lines came back.");
    Ok(())
}

/// Any endpoint, with the answer printed as JSON. Config var values are masked here too, so the
/// raw call is not a way around `hku config`.
pub fn call(
    client: &Client,
    method: &str,
    path: &str,
    body: Option<String>,
    yes: bool,
    reveal: bool,
) -> Result<()> {
    let method = Method::from_bytes(method.to_uppercase().as_bytes())
        .context("Method should be GET, POST, PATCH, PUT or DELETE")?;
    let path = format!("/{}", path.trim_start_matches("https://api.heroku.com").trim_start_matches('/'));
    let secret = !reveal && SECRET_PATHS.iter().any(|end| path.trim_end_matches('/').ends_with(end));
    let shown = |answer: Value| view::json(&if secret { masked(answer) } else { answer });
    if method == Method::GET {
        return shown(client.get_all(&path, None, usize::MAX)?);
    }
    let body: Option<Value> = match body {
        Some(text) => Some(serde_json::from_str(&text).context("The body is not valid JSON")?),
        None => None,
    };
    if !yes && !confirm(&method, &path, body.as_ref())? {
        return Ok(());
    }
    shown(client.send(method, &path, body.as_ref())?)
}

/// Anything but a GET changes a live app, so it is shown and asked about first. With nobody at
/// the keyboard there is no one to ask, and it stops rather than guess.
fn confirm(method: &Method, path: &str, body: Option<&Value>) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!(
            "{method} {path} changes something on Heroku, and there is nobody at this keyboard to confirm it.\n\
             Re-run it with --yes once the owner has said yes to this exact call."
        );
    }
    println!();
    println!("  {} {} {}", style("About to send").yellow().bold(), style(method.as_str()).bold(), path);
    if let Some(body) = body {
        println!("{}", serde_json::to_string_pretty(body)?);
    }
    Ok(dialoguer::Confirm::new().with_prompt("Go ahead?").default(false).interact()?)
}

#[cfg(test)]
mod tests {
    use super::masked;
    use serde_json::json;

    #[test]
    fn config_vars_and_addon_config_come_back_with_every_value_masked() {
        let vars = masked(json!({ "DATABASE_URL": "postgres://u:p@h/db", "UNSET": null }));
        assert_eq!(vars, json!({ "DATABASE_URL": "•••• 19 chars", "UNSET": null }));

        let addon = masked(json!([{ "name": "URL", "value": "redis://x" }]));
        assert_eq!(addon, json!([{ "name": "URL", "value": "•••• 9 chars" }]));
    }
}
