mod ai;
mod client;
mod config;
mod forms;
mod models;
mod money;
mod providers;
mod setup;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Config) => show_config(),
        Some(Command::Model) => setup::choose_model(),
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

/// Natural-language entry point: the model proposes, Rust validates, the form
/// fills the gaps, and nothing is sent until it is shown and confirmed.
async fn ask(request: String) -> Result<()> {
    let cfg = config::load()?;
    if cfg.ai.provider.is_empty() {
        println!("\n  No AI provider set. Run `agree model` to pick one.\n");
        return Ok(());
    }

    let api = Client::new(&cfg)?;
    ui::print_header();

    let spinner = ui::spinner("Reading your request...");
    let intent = ai::read_intent(&cfg, &request).await;
    spinner.finish_and_clear();

    // A model that fails to answer usefully is not a dead end — fall through to
    // the form and let the user fill it in by hand.
    let intent = match intent {
        Ok(intent) => intent,
        Err(e) => {
            println!("  {}\n", e);
            ai::Intent { action: "create_invoice".into(), ..Default::default() }
        }
    };

    if let Some(unclear) = intent.unclear.as_deref().filter(|u| !u.is_empty()) {
        println!("  unclear: {unclear}");
    }

    match intent.action.as_str() {
        "create_invoice" => create_invoice(&api, &cfg, &intent).await,
        "list_invoices" => {
            let filters = intent
                .status
                .clone()
                .map(|s| vec![("statuses".to_string(), s)])
                .unwrap_or_default();
            let rows: Vec<Invoice> = api.list_all("/api/v1/invoices", &filters, 20).await?;
            ui::print_invoices(&rows, false)
        }
        "find_contact" => {
            let rows: Vec<Contact> = api.list_all("/api/v1/contacts", &[], 0).await?;
            let rows = match intent.payee.as_deref() {
                Some(q) => rows.into_iter().filter(|c| c.matches(q)).collect(),
                None => rows,
            };
            ui::print_contacts(&rows, false)
        }
        other => {
            println!("  Not sure what to do with that (action: {other}).");
            println!("  Try `agree --help` for the direct commands.\n");
            Ok(())
        }
    }
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
        Command::Config | Command::Model => unreachable!("handled before a client is built"),
    }
}
