mod client;
mod config;
mod dashboards;
mod spec;
mod view;
mod wizard;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use client::Client;
use config::Config;
use console::style;
use reqwest::Method;
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "phog")]
#[command(about = "PostHog from the terminal — HogQL, people, events, and dashboards as files", long_about = None)]
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
    /// Show where config lives and whether a key and project are set
    Config,
    /// Take a key and a project id, check them against the API, and save them
    Setup,
    /// Run HogQL: phog query "select event, count() from events where timestamp > now() - interval 1 day group by event"
    Query {
        /// The HogQL. Read from stdin when omitted.
        hogql: Option<String>,
    },
    /// Everything PostHog holds on one person, newest first
    Events {
        /// The device fingerprint the website stamps on every event
        #[arg(long)]
        fingerprint: Option<String>,
        /// A person's email
        #[arg(long)]
        email: Option<String>,
        /// A PostHog distinct id
        #[arg(long)]
        distinct_id: Option<String>,
        /// How far back, as PostHog writes it
        #[arg(long, default_value = "-90d")]
        since: String,
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    /// Find people by email, name, distinct id or fingerprint
    Persons {
        search: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List dashboards, or work on one
    Dashboards {
        #[command(subcommand)]
        command: Option<DashboardCommand>,
    },
    /// Call any endpoint directly: phog call GET insights/ ; phog call PATCH dashboards/12/ '{"name":"x"}'
    Call {
        /// GET, POST, PATCH or DELETE
        method: String,
        /// Path under /api/projects/<id>/, or a full /api/… path
        path: String,
        /// A JSON body, for POST and PATCH
        body: Option<String>,
        /// Skip the confirmation on anything that writes
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum DashboardCommand {
    /// Every dashboard in the project
    List,
    /// One dashboard and its tiles, by id or name
    Show { dashboard: String },
    /// Make PostHog match a dashboard file
    Apply {
        file: String,
        /// Say what would change and stop
        #[arg(long)]
        dry_run: bool,
        /// Leave managed insights that are no longer in the file where they are
        #[arg(long)]
        no_prune: bool,
    },
    /// What apply would change, without changing it
    Diff { file: String },
    /// Parse a dashboard file and print what each insight would send. Needs no key.
    Check { file: String },
    /// Write a dashboard PostHog holds to a file that apply can read back
    Export {
        dashboard: String,
        /// File to write; stdout when omitted
        #[arg(long)]
        out: Option<String>,
    },
    /// Delete a dashboard. Its insights are left in place.
    Delete {
        dashboard: String,
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load()?;

    match cli.command {
        None => wizard::run(&config),
        Some(Command::Config) => show_config(&config),
        Some(Command::Setup) => wizard::setup(&config).map(|_| ()),
        Some(Command::Query { hogql }) => {
            let client = Client::new(&config)?;
            let hogql = match hogql {
                Some(text) => text,
                None => {
                    let mut text = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
                    text
                }
            };
            if hogql.trim().is_empty() {
                bail!("Nothing to run. Pass the HogQL as an argument or on stdin.");
            }
            view::query_result(&client.query(&hogql)?, cli.json)
        }
        Some(Command::Events { fingerprint, email, distinct_id, since, limit }) => {
            let client = Client::new(&config)?;
            events(&client, fingerprint, email, distinct_id, &since, limit, cli.json)
        }
        Some(Command::Persons { search, limit }) => {
            let client = Client::new(&config)?;
            persons(&client, &search, limit, cli.json)
        }
        Some(Command::Dashboards { command: Some(DashboardCommand::Check { file }) }) => {
            check(&file, cli.json)
        }
        Some(Command::Dashboards { command }) => {
            let client = Client::new(&config)?;
            match command.unwrap_or(DashboardCommand::List) {
                DashboardCommand::List => view::dashboards(&dashboards::list(&client)?, cli.json),
                DashboardCommand::Show { dashboard } => show(&client, &dashboard, cli.json),
                DashboardCommand::Apply { file, dry_run, no_prune } => {
                    apply(&client, &file, dry_run, !no_prune)
                }
                DashboardCommand::Diff { file } => apply(&client, &file, true, true),
                DashboardCommand::Export { dashboard, out } => export(&client, &dashboard, out),
                DashboardCommand::Delete { dashboard, yes } => delete(&client, &dashboard, yes),
                DashboardCommand::Check { file } => check(&file, cli.json),
            }
        }
        Some(Command::Call { method, path, body, yes }) => {
            let client = Client::new(&config)?;
            call(&client, &method, &path, body, yes)
        }
    }
}

pub fn show_config(config: &Config) -> Result<()> {
    println!();
    println!("  Config file : {}", config::config_path()?.display());
    println!("  App host    : {}", config.host);
    println!(
        "  Project     : {}",
        if config.project_id.is_empty() {
            style("not set").red().to_string()
        } else {
            config.project_id.clone()
        }
    );
    println!(
        "  API key     : {}",
        if config.api_key.is_empty() {
            style("not set").red().to_string()
        } else {
            format!("set ({} chars)", config.api_key.len())
        }
    );
    println!();
    if config.api_key.is_empty() || config.project_id.is_empty() {
        println!("{}\n", config::missing_help());
    }
    Ok(())
}

/// One person's timeline, by whichever handle you have. Fingerprint reaches
/// across cookies and the agent's server-side events; the others are exact.
pub fn events(
    client: &Client,
    fingerprint: Option<String>,
    email: Option<String>,
    distinct_id: Option<String>,
    since: &str,
    limit: usize,
    as_json: bool,
) -> Result<()> {
    let clause = match (fingerprint, email, distinct_id) {
        (Some(fp), _, _) => format!(
            "(properties.fingerprint = {lit} OR distinct_id = {fp_id})",
            lit = client::literal(&fp),
            fp_id = client::literal(&format!("fp:{fp}"))
        ),
        (_, Some(email), _) => format!("person.properties.email = {}", client::literal(&email)),
        (_, _, Some(id)) => format!("distinct_id = {}", client::literal(&id)),
        _ => bail!("Say who: --fingerprint, --email or --distinct-id"),
    };
    let hogql = format!(
        "SELECT timestamp, event, distinct_id, properties.$pathname AS path, properties.source AS source, properties.plan_chat_id AS chat FROM events WHERE {clause} AND timestamp > now() - INTERVAL {} ORDER BY timestamp DESC LIMIT {limit}",
        interval_of(since)
    );
    let result = client.query(&hogql)?;
    view::query_result(&result, as_json)?;
    if !as_json {
        let ids: Vec<String> = result
            .rows
            .iter()
            .filter_map(|r| r.get(2).and_then(Value::as_str).map(str::to_string))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for id in ids.iter().take(5) {
            view::note(&format!("{}  {}", id, client.person_url(id)));
        }
        println!();
    }
    Ok(())
}

/// `-90d` → `90 DAY`, `-24h` → `24 HOUR`, so `--since` reads like PostHog's own date ranges.
fn interval_of(since: &str) -> String {
    let text = since.trim().trim_start_matches('-');
    let (digits, unit) = text.split_at(text.trim_end_matches(|c: char| c.is_ascii_alphabetic()).len());
    let number: u64 = digits.parse().unwrap_or(90);
    let unit = match unit {
        "h" => "HOUR",
        "w" => "WEEK",
        "m" | "M" => "MONTH",
        "y" => "YEAR",
        _ => "DAY",
    };
    format!("{number} {unit}")
}

pub fn persons(client: &Client, search: &str, limit: usize, as_json: bool) -> Result<()> {
    let path = client.project_path("persons/");
    // A fingerprint is a person property, and the search box does not look at those
    let fingerprint_filter =
        json!([{ "key": "fingerprint", "value": search, "operator": "exact", "type": "person" }]).to_string();
    let by_search = client.get(&path, &[("search", search.to_string()), ("limit", limit.to_string())])?;
    let by_fingerprint =
        client.get(&path, &[("properties", fingerprint_filter), ("limit", limit.to_string())])?;
    let mut people: Vec<Value> = Vec::new();
    for page in [by_search, by_fingerprint] {
        if let Some(results) = page.get("results").and_then(Value::as_array) {
            for person in results {
                if !people.iter().any(|p| p.get("id") == person.get("id")) {
                    people.push(person.clone());
                }
            }
        }
    }
    if as_json {
        return view::json(&Value::Array(people));
    }
    if people.is_empty() {
        println!("\n  {}\n", style("Nobody matches that.").dim());
        return Ok(());
    }
    println!();
    for person in &people {
        let props = person.get("properties").cloned().unwrap_or(Value::Null);
        let email = props.get("email").and_then(Value::as_str).unwrap_or("—");
        let fingerprint = props.get("fingerprint").and_then(Value::as_str).unwrap_or("—");
        let ids: Vec<&str> = person
            .get("distinct_ids")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        println!("  {}  {}", style(email).bold(), style(fingerprint).dim());
        for id in ids.iter().take(4) {
            println!("    {:<40} {}", id, style(client.person_url(id)).dim());
        }
    }
    println!();
    Ok(())
}

fn show(client: &Client, dashboard: &str, as_json: bool) -> Result<()> {
    let id = dashboards::resolve(client, dashboard)?;
    if as_json {
        return view::json(&client.get(&client.project_path(&format!("dashboards/{id}/")), &[])?);
    }
    let remote = dashboards::read(client, id)?;
    println!();
    println!("  {}  {}", style(&remote.name).bold(), style(client.dashboard_url(id)).dim());
    if !remote.description.is_empty() {
        println!("  {}", remote.description);
    }
    if !remote.tags.is_empty() {
        println!("  {}", style(remote.tags.join(", ")).cyan());
    }
    println!();
    for tile in &remote.tiles {
        let kind = tile
            .query
            .pointer("/source/kind")
            .or_else(|| tile.query.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let layout = tile.layout.map(|l| format!("{}×{} at {},{}", l.w, l.h, l.x, l.y)).unwrap_or_default();
        let key =
            spec::slug_of(&remote.tags).and_then(|slug| spec::key_of(&tile.tags, &slug)).unwrap_or_default();
        println!(
            "  {:<6} {:<40} {:<14} {:<14} {}",
            tile.insight_id,
            view::cell(&Value::String(tile.name.clone()), 40),
            style(kind).dim(),
            style(layout).dim(),
            style(key).cyan()
        );
    }
    println!();
    Ok(())
}

pub fn apply(client: &Client, file: &str, dry_run: bool, prune: bool) -> Result<()> {
    let text = std::fs::read_to_string(file).with_context(|| format!("Could not read {file}"))?;
    let dashboard = spec::parse(&text)?;
    let (_, changes) = dashboards::plan(client, &dashboard)?;

    println!();
    println!(
        "  {}  {}",
        style(&dashboard.name).bold(),
        style(format!(
            "{} insight{}",
            dashboard.insights.len(),
            if dashboard.insights.len() == 1 { "" } else { "s" }
        ))
        .dim()
    );
    let changes: Vec<_> =
        changes.into_iter().filter(|c| prune || !matches!(c, dashboards::Change::RemoveInsight(_))).collect();
    if changes.is_empty() {
        println!("  {} Nothing to change.\n", style("✓").green());
        return Ok(());
    }
    for change in &changes {
        println!("  {change}");
    }
    println!();
    if dry_run {
        view::note("Dry run — nothing was changed.");
        println!();
        return Ok(());
    }
    dashboards::apply(client, &dashboard, prune)?;
    println!();
    Ok(())
}

/// The file, read the way apply reads it, with nothing sent anywhere.
fn check(file: &str, as_json: bool) -> Result<()> {
    let text = std::fs::read_to_string(file).with_context(|| format!("Could not read {file}"))?;
    let dashboard = spec::parse(&text)?;
    if as_json {
        let insights: Result<Vec<Value>> = dashboard
            .insights
            .iter()
            .map(|insight| Ok(json!({ "key": insight.key, "name": insight.name, "tags": insight.all_tags(&dashboard), "query": insight.to_query(&dashboard)? })))
            .collect();
        return view::json(
            &json!({ "name": dashboard.name, "tag": dashboard.tag(), "tags": dashboard.all_tags(), "insights": insights? }),
        );
    }
    println!();
    println!("  {}  {}", style(&dashboard.name).bold(), style(dashboard.tag()).cyan());
    for insight in &dashboard.insights {
        let query = insight.to_query(&dashboard)?;
        let kind = query
            .pointer("/source/kind")
            .or_else(|| query.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let layout = insight
            .layout
            .map(|l| format!("{}×{} at {},{}", l.w, l.h, l.x, l.y))
            .unwrap_or_else(|| "auto".to_string());
        println!(
            "  {:<18} {:<40} {:<14} {}",
            style(&insight.key).cyan(),
            view::cell(&Value::String(insight.name.clone()), 40),
            style(kind).dim(),
            style(layout).dim()
        );
    }
    println!();
    println!(
        "  {} {} insights parse. `--json` prints every query node.",
        style("✓").green(),
        dashboard.insights.len()
    );
    println!();
    Ok(())
}

pub fn export(client: &Client, dashboard: &str, out: Option<String>) -> Result<()> {
    let id = dashboards::resolve(client, dashboard)?;
    let spec = dashboards::export(client, id)?;
    let yaml = serde_yaml::to_string(&spec)?;
    match out {
        Some(path) => {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&path, &yaml).with_context(|| format!("Could not write {path}"))?;
            println!("\n  {} wrote {} ({} insights)\n", style("✓").green(), path, spec.insights.len());
        }
        None => print!("{yaml}"),
    }
    Ok(())
}

fn delete(client: &Client, dashboard: &str, yes: bool) -> Result<()> {
    let id = dashboards::resolve(client, dashboard)?;
    let remote = dashboards::read(client, id)?;
    if !yes {
        println!();
        let ok = dialoguer::Confirm::new()
            .with_prompt(format!("Delete dashboard {} ({} tiles)?", remote.name, remote.tiles.len()))
            .default(false)
            .interact()?;
        if !ok {
            return Ok(());
        }
    }
    dashboards::delete(client, id)?;
    println!("  {} deleted {}\n", style("✓").red(), remote.name);
    Ok(())
}

fn call(client: &Client, method: &str, path: &str, body: Option<String>, yes: bool) -> Result<()> {
    let method = Method::from_bytes(method.to_uppercase().as_bytes())
        .context("Method should be GET, POST, PATCH or DELETE")?;
    let path = if path.starts_with("/api/") { path.to_string() } else { client.project_path(path) };
    let body: Option<Value> = match body {
        Some(text) => Some(serde_json::from_str(&text).context("The body is not valid JSON")?),
        None => None,
    };
    if method != Method::GET && !yes {
        println!();
        println!("  {} {} {}", style("About to send").yellow().bold(), style(method.as_str()).bold(), path);
        if let Some(body) = &body {
            println!("{}", serde_json::to_string_pretty(body)?);
        }
        if !dialoguer::Confirm::new().with_prompt("Go ahead?").default(false).interact()? {
            return Ok(());
        }
    }
    view::json(&client.send(method, &path, body.as_ref())?)
}
