#![allow(dead_code)]

mod api;
mod auth;
mod cli;
mod client;
mod config;
mod error;
mod ui;

use crate::client::GoogleClient;
use crate::config::{Account, AuthConfig, Config};
use crate::ui::app::{App, Screen};
use crate::ui::views::handlers;
use crate::ui::views::render;

use clap::Parser;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_args = cli::Cli::parse();

    if let Some(command) = cli_args.command {
        return run_cli(command).await;
    }

    let config = if Config::exists() {
        let mut cfg = Config::load()?;
        if cfg.accounts.is_empty() {
            println!("\n  No accounts configured. Let's add one.\n");
            let (name, account) = add_account_flow().await?;
            cfg.add_account(name.clone(), account);
            cfg.active_account = name;
            cfg.save()?;
        }
        cfg
    } else {
        first_time_setup().await?
    };

    if !config.active_is_valid() {
        eprintln!("  Active account has incomplete credentials.");
        eprintln!("  Run vgoog and add a new account.\n");
        let mut cfg = config;
        let (name, account) = add_account_flow().await?;
        cfg.add_account(name.clone(), account);
        cfg.active_account = name;
        cfg.save()?;
        run_tui(cfg).await?;
    } else {
        run_tui(config).await?;
    }

    Ok(())
}

// ── First-time setup ──

async fn first_time_setup() -> anyhow::Result<Config> {
    print_banner("First-time Setup");

    let (name, account) = add_account_flow().await?;

    let mut config = Config {
        active_account: name.clone(),
        accounts: Default::default(),
        auth: None,
    };
    config.add_account(name, account);
    config.save()?;

    println!("\n  Config saved! Starting vgoog...\n");
    Ok(config)
}

// ── Add account flow ──

async fn add_account_flow() -> anyhow::Result<(String, Account)> {
    let name = prompt("  Account name (e.g. work, personal): ")?;
    let label = prompt("  Display label (e.g. Work Gmail): ")?;

    let auth = manual_token_flow()?;

    let account_name = if name.is_empty() {
        "default".to_string()
    } else {
        name.to_lowercase().replace(' ', "-")
    };

    let account_label = if label.is_empty() {
        account_name.clone()
    } else {
        label
    };

    Ok((
        account_name,
        Account {
            label: account_label,
            auth,
        },
    ))
}

// ── Manual Token Flow ──

fn manual_token_flow() -> anyhow::Result<AuthConfig> {
    println!("\n  ── Manual Token Entry ──\n");
    println!("  Get tokens from: https://console.cloud.google.com/apis/credentials");
    println!("  Or use the OAuth Playground: https://developers.google.com/oauthplayground/\n");

    let client_id = prompt("  GOOGLE_OAUTH_CLIENT_ID: ")?;
    let client_secret = prompt("  GOOGLE_OAUTH_CLIENT_SECRET: ")?;
    let access_token = prompt("  GOOGLE_OAUTH_ACCESS_TOKEN: ")?;
    let refresh_token = prompt("  GOOGLE_OAUTH_REFRESH_TOKEN: ")?;

    Ok(AuthConfig {
        client_id,
        client_secret,
        access_token,
        refresh_token,
        token_expiry: chrono::Utc::now(),
    })
}

// ── Account management (pre-TUI) ──

async fn manage_accounts_menu(config: &mut Config) -> anyhow::Result<bool> {
    println!("\n  ── Account Management ──\n");
    println!("  Active: {} ({})", config.active_account, config.active().map(|a| a.label.as_str()).unwrap_or("?"));
    println!();

    for (i, (name, account)) in config.accounts.iter().enumerate() {
        let marker = if *name == config.active_account { " *" } else { "  " };
        println!("  {}{} {} — {}", marker, i + 1, name, account.label);
    }

    println!("\n  [a] Add account  [r] Remove account  [s] Switch account  [q] Continue\n");

    let choice = prompt("  Choice: ")?;
    match choice.trim() {
        "a" => {
            let (name, account) = add_account_flow().await?;
            config.add_account(name.clone(), account);
            config.active_account = name;
            config.save()?;
            println!("  Account added and activated!");
            Ok(true)
        }
        "r" => {
            if config.accounts.len() <= 1 {
                println!("  Cannot remove the only account.");
                return Ok(true);
            }
            let name = prompt("  Account name to remove: ")?;
            if config.remove_account(&name) {
                config.save()?;
                println!("  Account '{}' removed.", name);
            } else {
                println!("  Account '{}' not found.", name);
            }
            Ok(true)
        }
        "s" => {
            let name = prompt("  Account name to switch to: ")?;
            if config.switch_account(&name) {
                config.save()?;
                println!("  Switched to '{}'.", name);
            } else {
                println!("  Account '{}' not found.", name);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── TUI ──

async fn run_tui(config: Config) -> anyhow::Result<()> {
    let client = GoogleClient::new(config)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(client);

    // Load account info into app
    app.account_name = app.client.active_account_name().await;
    app.account_label = app.client.active_account_label().await;
    app.account_list = app.client.account_names().await;

    loop {
        terminal.draw(|f| render::render(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    break;
                }

                // Account switcher: Ctrl+A
                if key.code == KeyCode::Char('a')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && app.screen != Screen::Input
                {
                    app.show_account_switcher = !app.show_account_switcher;
                    if app.show_account_switcher {
                        app.account_list = app.client.account_names().await;
                        app.account_cursor = 0;
                        app.set_status("Switch account: ↑↓ select, Enter confirm, Esc cancel");
                    }
                    continue;
                }

                // Handle account switcher overlay
                if app.show_account_switcher {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.account_cursor > 0 {
                                app.account_cursor -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.account_cursor < app.account_list.len().saturating_sub(1) {
                                app.account_cursor += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(name) = app.account_list.get(app.account_cursor).cloned() {
                                match app.client.switch_account(&name).await {
                                    Ok(()) => {
                                        app.account_name = name.clone();
                                        app.account_label =
                                            app.client.active_account_label().await;
                                        app.set_status(format!("Switched to {}", app.account_label));
                                        // Reset view state
                                        app.screen = Screen::ServiceSelect;
                                        app.items.clear();
                                        app.detail = None;
                                        app.service = None;
                                    }
                                    Err(e) => {
                                        app.set_status(format!("Error: {e}"));
                                    }
                                }
                            }
                            app.show_account_switcher = false;
                        }
                        KeyCode::Esc => {
                            app.show_account_switcher = false;
                            app.set_status("Account switch cancelled");
                        }
                        _ => {}
                    }
                    continue;
                }

                if key.code == KeyCode::Char('q') && app.screen != Screen::Input {
                    if app.screen == Screen::ServiceSelect {
                        break;
                    }
                    app.go_back();
                    continue;
                }

                match app.screen {
                    Screen::ServiceSelect => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        KeyCode::Enter => {
                            app.service = Some(app.current_service());
                            app.selected_action = 0;
                            app.screen = Screen::ActionSelect;
                            app.set_status(format!(
                                "Selected {}. Choose an action.",
                                app.current_service().name()
                            ));
                        }
                        KeyCode::Esc => break,
                        _ => {}
                    },
                    Screen::ActionSelect => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        KeyCode::Enter => {
                            app.set_status("Loading...");
                            handlers::execute_action(&mut app).await;
                        }
                        KeyCode::Esc => app.go_back(),
                        _ => {}
                    },
                    Screen::ActionView => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.detail.is_some() {
                                app.scroll_detail_up();
                            } else {
                                app.move_up();
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.detail.is_some() {
                                app.scroll_detail_down();
                            } else {
                                app.move_down();
                            }
                        }
                        KeyCode::Enter => {
                            if app.detail.is_none() {
                                handlers::execute_detail(&mut app).await;
                            }
                        }
                        KeyCode::Char('d') => {
                            if app.detail.is_none() && !app.items.is_empty() {
                                if let Some(item) = app.current_item() {
                                    app.confirm_message = format!("Delete '{}'?", item.title);
                                    app.screen = Screen::Confirm;
                                }
                            }
                        }
                        KeyCode::Char('n') => {
                            handlers::load_next_page(&mut app).await;
                        }
                        KeyCode::Esc => {
                            if app.detail.is_some() {
                                app.detail = None;
                                app.scroll_offset = 0;
                            } else {
                                app.go_back();
                            }
                        }
                        _ => {}
                    },
                    Screen::Input => match key.code {
                        KeyCode::Tab => {
                            app.input_field_cursor =
                                (app.input_field_cursor + 1) % app.input_fields.len().max(1);
                        }
                        KeyCode::BackTab => {
                            if app.input_field_cursor > 0 {
                                app.input_field_cursor -= 1;
                            } else {
                                app.input_field_cursor = app.input_fields.len().saturating_sub(1);
                            }
                        }
                        KeyCode::Enter => {
                            let all_valid = app
                                .input_fields
                                .iter()
                                .all(|f| !f.required || !f.value.is_empty());
                            if all_valid {
                                handlers::submit_input(&mut app).await;
                            } else {
                                app.set_status("Please fill in all required fields (*)");
                            }
                        }
                        KeyCode::Esc => app.go_back(),
                        KeyCode::Backspace => {
                            if let Some(field) = app.input_fields.get_mut(app.input_field_cursor) {
                                field.value.pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            if let Some(field) = app.input_fields.get_mut(app.input_field_cursor) {
                                field.value.push(c);
                            }
                        }
                        _ => {}
                    },
                    Screen::Confirm => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            app.screen = Screen::ActionView;
                            handlers::execute_delete(&mut app).await;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.screen = Screen::ActionView;
                            app.set_status("Cancelled");
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    println!("Thanks for using vgoog!");
    Ok(())
}

// ── CLI mode ──

async fn run_cli(command: cli::CliCommand) -> anyhow::Result<()> {
    match command {
        cli::CliCommand::Exec { service, action, args, account } => {
            let mut config = Config::load()?;

            if let Some(ref acct_name) = account {
                if !config.switch_account(acct_name) {
                    eprintln!("{}", serde_json::to_string(&serde_json::json!({
                        "ok": false,
                        "error": format!("Account '{}' not found", acct_name)
                    }))?);
                    std::process::exit(1);
                }
            }

            let client = GoogleClient::new(config)?;

            let parsed_args: serde_json::Value = args
                .map(|s| serde_json::from_str(&s))
                .transpose()?
                .unwrap_or(serde_json::json!({}));

            match cli::exec::execute(&client, &service, &action, parsed_args).await {
                Ok(val) => {
                    println!("{}", serde_json::to_string(&serde_json::json!({
                        "ok": true,
                        "data": val
                    }))?);
                }
                Err(e) => {
                    eprintln!("{}", serde_json::to_string(&serde_json::json!({
                        "ok": false,
                        "error": e.to_string()
                    }))?);
                    std::process::exit(1);
                }
            }
        }
        cli::CliCommand::List => {
            println!("{}", serde_json::to_string_pretty(&cli::exec::list_all())?);
        }
        cli::CliCommand::Status => {
            let config = Config::load()?;
            let client = GoogleClient::new(config)?;
            let api = api::gmail::GmailApi::new(&client);

            match api.get_profile().await {
                Ok(val) => {
                    println!("{}", serde_json::to_string(&serde_json::json!({
                        "ok": true,
                        "data": val
                    }))?);
                }
                Err(e) => {
                    eprintln!("{}", serde_json::to_string(&serde_json::json!({
                        "ok": false,
                        "error": e.to_string()
                    }))?);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

// ── Utilities ──

fn print_banner(subtitle: &str) {
    println!("\n  ╔═══════════════════════════════════════╗");
    println!("  ║     vgoog — Google Workspace TUI      ║");
    println!("  ║     {:<33} ║", subtitle);
    println!("  ╚═══════════════════════════════════════╝\n");
}

fn prompt(msg: &str) -> anyhow::Result<String> {
    use std::io::Write;
    print!("{msg}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
