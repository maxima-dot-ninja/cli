mod agent;
mod ai;
mod client;
mod config;
mod forms;
mod models;
mod money;
mod providers;
mod setup;
mod tools;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use client::Client;
use models::{Agreement, Contact, Customer, Invoice, Template};

#[derive(Parser)]
#[command(name = "agree")]
#[command(about = "CLI for the Agree API — invoices, agreements, contacts", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Say what you want in plain English, e.g.
    /// agree "recurring invoice for Samir, $5000/week"
    #[arg(trailing_var_arg = true)]
    request: Vec<String>,

    /// Print raw JSON instead of a table
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Show where config lives and whether a key is set
    Config,
    /// Choose the AI provider and model
    Model,
    /// List and inspect invoices
    Invoices {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        customer: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List contacts, optionally filtered by a name/email fragment
    Contacts {
        query: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// List customers
    Customers {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List agreements
    Agreements {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List agreement templates
    Templates,
    /// List every API operation available to you and to the AI
    Tools,
    /// Call any API operation directly: agree call delete_invoice id=abc
    Call {
        /// Tool name, as shown by `agree tools`
        tool: String,
        /// key=value pairs
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Skip the confirmation prompt on changes
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Config) => show_config(),
        Some(Command::Model) => setup::choose_model(),
        Some(Command::Tools) => {
            println!("\n{}\n", tools::catalogue());
            Ok(())
        }
        Some(command) => run(command, cli.json).await,
        None if !cli.request.is_empty() => ask(cli.request.join(" ")).await,
        None => {
            ui::print_header();
            println!("  Run `agree --help` for commands, or `agree config` to check your key.");
            println!("  Or just say what you want:  agree \"invoice Samir $5000 a week\"\n");
            Ok(())
        }
    }
}

/// Natural-language entry point. The model plans, calls tools one at a time, and
/// stops for confirmation before anything that changes data.
async fn ask(request: String) -> Result<()> {
    let cfg = config::load()?;
    if cfg.ai.provider.is_empty() {
        println!("\n  No AI provider set. Run `agree model` to pick one.\n");
        return Ok(());
    }

    let api = Client::new(&cfg)?;
    ui::print_header();

    // Creating an invoice keeps its dedicated form: it resolves the payee, pins
    // the repeat to a weekday and converts dollars to cents — none of which the
    // generic tool path does.
    if looks_like_new_invoice(&request) {
        let intent = ai::read_intent(&cfg, &request).await?;
        if intent.action == "create_invoice" {
            return create_invoice(&api, &cfg, &intent).await;
        }
    }

    agent::run(&cfg, &api, &request).await
}

/// Cheap check so the common "make me an invoice" path skips a planning round trip.
fn looks_like_new_invoice(request: &str) -> bool {
    let text = request.to_lowercase();
    let makes = ["create", "make", "new", "draft", "bill ", "invoice "]
        .iter()
        .any(|word| text.contains(word));
    let existing = ["delete", "list", "show", "find", "how many", "when", "cancel", "send the", "mark"]
        .iter()
        .any(|word| text.contains(word));
    makes && !existing
}

async fn create_invoice(api: &Client, cfg: &config::Config, intent: &ai::Intent) -> Result<()> {
    println!();
    let Some(invoice) = forms::build_invoice(api, intent, &cfg.currency).await? else {
        println!("  Cancelled.\n");
        return Ok(());
    };

    let spinner = ui::spinner("Creating invoice...");
    let result = api.post_raw("/api/v1/invoices", &serde_json::json!({ "invoice": invoice })).await;
    spinner.finish_and_clear();

    let created = result?;
    let id = created
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("(no id)");
    ui::success(&format!("Created invoice {id}"));

    if let Some(url) = created.get("data").and_then(|d| d.get("invoice_url")).and_then(|v| v.as_str()) {
        println!("  {}\n", url);
    }
    Ok(())
}

/// Direct access to any operation, so nothing is reachable only through the AI.
/// Values are parsed as JSON when they look like it, so numbers and lists work.
async fn call_tool(api: &Client, name: &str, args: &[String], yes: bool) -> Result<()> {
    let Some(tool) = tools::find(name) else {
        println!("\n  Unknown tool: {name}");
        println!("  Run `agree tools` to see them all.\n");
        return Ok(());
    };

    let mut map = serde_json::Map::new();
    for pair in args {
        let Some((key, value)) = pair.split_once('=') else {
            println!("  Skipping `{pair}` — expected key=value");
            continue;
        };
        let parsed = serde_json::from_str(value)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
        map.insert(key.to_string(), parsed);
    }

    if tool.mutates && !yes {
        println!();
        println!("  {}", console::style("This will change your data:").yellow());
        for line in tools::describe_call(tool, &map).lines() {
            println!("  {line}");
        }
        println!();
        let go = dialoguer::Confirm::new().with_prompt("Go ahead?").default(false).interact()?;
        if !go {
            println!("  Cancelled.\n");
            return Ok(());
        }
    }

    let result = tools::run(api, tool, &map).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn show_config() -> Result<()> {
    let cfg = config::load()?;
    ui::print_header();
    println!("  Config file : {}", config::config_path()?.display());
    println!("  Base URL    : {}", config::BASE_URL);
    println!("  Currency    : {}", cfg.currency);
    let key = match cfg.api_key.is_empty() {
        true => "not set".to_string(),
        false => format!("set ({} chars)", cfg.api_key.len()),
    };
    println!("  API key     : {key}");
    println!();
    if cfg.api_key.is_empty() {
        println!("{}\n", config::missing_key_help());
    }
    Ok(())
}

async fn run(command: Command, as_json: bool) -> Result<()> {
    let cfg = config::load()?;
    let api = Client::new(&cfg)?;

    match command {
        Command::Invoices { status, customer, limit } => {
            let mut filters = Vec::new();
            if let Some(s) = status {
                filters.push(("statuses".to_string(), s));
            }
            if let Some(c) = customer {
                filters.push(("customer".to_string(), c));
            }
            let rows: Vec<Invoice> = api.list_all("/api/v1/invoices", &filters, limit).await?;
            ui::print_invoices(&rows, as_json)
        }
        Command::Contacts { query, limit } => {
            let rows: Vec<Contact> = api.list_all("/api/v1/contacts", &[], limit).await?;
            // The API filters contacts by email and company only, so a spoken
            // name like "Samir" has to be matched here.
            let rows = match &query {
                Some(q) => rows.into_iter().filter(|c| c.matches(q)).collect(),
                None => rows,
            };
            ui::print_contacts(&rows, as_json)
        }
        Command::Customers { limit } => {
            let rows: Vec<Customer> = api.list_all("/api/v1/customers", &[], limit).await?;
            ui::print_customers(&rows, as_json)
        }
        Command::Agreements { limit } => {
            let rows: Vec<Agreement> = api.list_all("/api/v1/agreements", &[], limit).await?;
            ui::print_agreements(&rows, as_json)
        }
        Command::Templates => {
            let rows: Vec<Template> = api.list_all("/api/v1/agreements/templates", &[], 0).await?;
            ui::print_templates(&rows, as_json)
        }
        Command::Call { tool, args, yes } => call_tool(&api, &tool, &args, yes).await,
        Command::Config | Command::Model | Command::Tools => {
            unreachable!("handled before a client is built")
        }
    }
}
