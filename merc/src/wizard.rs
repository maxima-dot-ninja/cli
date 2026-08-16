//! What `merc` does when you run it with nothing to go on.
//!
//! Every path here is also reachable non-interactively — the wizard only picks
//! the arguments, then hands them to the same code a flag would have reached.

use crate::client::Client;
use crate::config::Config;
use crate::money::Money;
use crate::ops::{self, Op, Param};
use crate::view;
use anyhow::{bail, Result};
use console::style;
use dialoguer::{Confirm, FuzzySelect, Input, Password, Select};
use serde_json::{Map, Value};

pub fn header(config: &Config) {
    let environment = match config.sandbox {
        true => style(" sandbox ").black().on_yellow().to_string(),
        false => style(" production ").white().on_red().to_string(),
    };
    println!();
    println!("  {}  {environment}", style("merc").cyan().bold());
    println!();
}

/// Show exactly what is about to happen, then ask. Nothing that changes money
/// runs without passing through here, unless `--yes` says the caller already
/// knows.
pub fn confirm(config: &Config, op: &'static Op, args: &Map<String, Value>, body: &Value) -> Result<bool> {
    let path = op.resolve_path(args)?;
    println!();
    println!("  {}", style("About to change data:").yellow().bold());
    println!("  {} {}{}", style(op.method.label()).bold(), config.base_url(), path);
    println!("  environment: {}", environment_label(config));
    if let Some(object) = body.as_object() {
        if !object.is_empty() {
            for line in view::pretty(body).lines() {
                println!("  {line}");
            }
        }
    }
    println!();
    Ok(Confirm::new().with_prompt("Go ahead?").default(false).interact()?)
}

fn environment_label(config: &Config) -> String {
    match config.sandbox {
        true => style("sandbox").yellow().to_string(),
        false => style("PRODUCTION — real money").red().bold().to_string(),
    }
}

/// First run: take a token, prove it works, then write it down.
///
/// Checking before saving is the point — a token pasted with its `secret-token:`
/// prefix trimmed off, or a sandbox token aimed at production, fails in a way
/// that is obvious now and baffling later.
pub fn setup(config: &Config) -> Result<Option<Config>> {
    println!("{}\n", crate::config::missing_key_help());
    if !Confirm::new().with_prompt("Paste a token now?").default(true).interact()? {
        return Ok(None);
    }

    let mut config = config.clone();
    config.api_key = Password::new().with_prompt("API token").interact()?.trim().to_string();
    config.sandbox = Select::new()
        .with_prompt("Which Mercury is this token for")
        .items(&["production — real money", "sandbox — test data"])
        .default(0)
        .interact()?
        == 1;

    let spinner = view::spinner("Checking the token…");
    let check =
        Client::new(&config)?.run(ops::find("accounts", "list").expect("accounts list exists"), &Map::new());
    spinner.finish_and_clear();
    check?;

    crate::config::save(&config)?;
    view::success(&format!("Saved to {}\n", crate::config::config_path()?.display()));
    Ok(Some(config))
}

/// Pick an operation by hand: group, then verb, then its arguments.
pub fn browse(config: &Config) -> Result<Option<(&'static Op, Map<String, Value>)>> {
    let groups = ops::groups();
    let labels: Vec<String> = groups
        .iter()
        .map(|group| {
            format!("{:<20} {}", group, style(format!("{} commands", ops::in_group(group).len())).dim())
        })
        .collect();

    let Some(chosen) =
        Select::new().with_prompt("What are you working with").items(&labels).default(0).interact_opt()?
    else {
        return Ok(None);
    };

    let commands = ops::in_group(groups[chosen]);
    let labels: Vec<String> = commands
        .iter()
        .map(|op| {
            let mark = match op.mutates() {
                true => style("•").yellow().to_string(),
                false => " ".to_string(),
            };
            format!("{mark} {:<24} {}", op.verb, style(op.about).dim())
        })
        .collect();

    let Some(chosen) = Select::new().with_prompt("Which command").items(&labels).default(0).interact_opt()?
    else {
        return Ok(None);
    };
    let op = commands[chosen];

    let args = ask_for(config, op)?;
    Ok(Some((op, args)))
}

/// Ask for what the operation cannot run without, then offer the rest.
fn ask_for(config: &Config, op: &'static Op) -> Result<Map<String, Value>> {
    let mut args = Map::new();
    for param in op.params.iter().filter(|p| p.required) {
        if let Some(value) = ask_one(config, op, param, &args)? {
            args.insert(param.name.to_string(), value);
        }
    }

    let optional: Vec<&Param> = op.params.iter().filter(|p| !p.required).collect();
    if optional.is_empty()
        || !Confirm::new().with_prompt("Set any optional arguments?").default(false).interact()?
    {
        return Ok(args);
    }

    let labels: Vec<String> =
        optional.iter().map(|p| format!("{:<22} {}", p.name, style(hint(p)).dim())).collect();
    let chosen = dialoguer::MultiSelect::new().with_prompt("Which ones").items(&labels).interact()?;
    for index in chosen {
        if let Some(value) = ask_one(config, op, optional[index], &args)? {
            args.insert(optional[index].name.to_string(), value);
        }
    }
    Ok(args)
}

/// One argument, asked in whatever way suits its type — a list to choose from
/// beats a prompt asking you to paste an id you would have to go and find.
fn ask_one(
    config: &Config,
    op: &'static Op,
    param: &Param,
    known: &Map<String, Value>,
) -> Result<Option<Value>> {
    if param.name == "idempotencyKey" {
        return Ok(None); // filled in automatically, and shown before sending
    }
    if let Some(picked) = pick_id(config, param, known, 0)? {
        return Ok(Some(picked));
    }
    if !param.choices.is_empty() {
        let chosen = Select::new().with_prompt(param.name).items(param.choices).default(0).interact()?;
        return Ok(Some(Value::from(param.choices[chosen])));
    }

    let prompt = match param.about.is_empty() {
        true => param.name.to_string(),
        false => format!("{} — {}", param.name, hint(param)),
    };
    let typed: String = Input::new().with_prompt(prompt).allow_empty(!param.required).interact_text()?;
    if typed.is_empty() {
        return Ok(None);
    }
    Ok(Some(crate::parse_value(op, param, &typed)?))
}

/// Ask for whatever is missing, instead of failing with a name to go and look up.
///
/// Only called when there is a terminal to ask; in a pipe the missing argument
/// is still an error, because a script waiting on a prompt nobody can see is
/// worse than a script that stops.
pub fn fill_missing(config: &Config, op: &'static Op, args: &mut Map<String, Value>) -> Result<()> {
    for param in op.missing(args) {
        let known = args.clone();
        if let Some(value) = ask_one(config, op, param, &known)? {
            args.insert(param.name.to_string(), value);
        }
    }
    Ok(())
}

/// A prompt is one line. Mercury's descriptions run to a paragraph, and
/// dialoguer echoes every chosen label back, so a full one turns a four-item
/// menu into a screen of text.
fn hint(param: &Param) -> String {
    let first = param.about.split(". ").next().unwrap_or_default().trim();
    match first.chars().count() > 56 {
        true => format!("{}…", first.chars().take(55).collect::<String>()),
        false => first.to_string(),
    }
}

/// Which listing answers "what could this id be".
///
/// Nobody has an account id written down, so nothing should ever ask for one.
/// Every id-shaped argument in the API is on this list, and a test fails if
/// Mercury adds one that is not.
const LOOKUPS: &[(&str, &str, &str)] = &[
    ("accountId", "accounts", "list"),
    ("attachmentId", "invoices", "list-attachments"),
    ("cardId", "cards", "list"),
    ("categoryId", "categories", "list"),
    ("customerId", "customers", "list"),
    ("destinationAccountId", "accounts", "list"),
    ("eventId", "events", "list"),
    ("expenseCategoryId", "categories", "list"),
    ("inviteId", "recipient-invites", "list"),
    ("invoiceId", "invoices", "list"),
    ("recipientId", "recipients", "list"),
    ("requestId", "send-money", "list-approval-requests"),
    ("safeRequestId", "safes", "list"),
    ("sourceAccountId", "accounts", "list"),
    ("statementId", "accounts", "get-statements"),
    ("transactionId", "transactions", "list"),
    ("treasuryId", "treasury", "list"),
    ("userId", "users", "list"),
    ("webhookEndpointId", "webhooks", "list"),
];

pub fn lookup_for(name: &str) -> Option<&'static Op> {
    let (_, group, verb) = LOOKUPS.iter().find(|(param, _, _)| *param == name)?;
    ops::find(group, verb)
}

/// Offer the real things instead of asking for an id.
///
/// A listing can need an id of its own — statements belong to an account — so
/// this calls itself for those first. `known` carries what has already been
/// chosen, so an account picked a moment ago is not asked for twice.
fn pick_id(config: &Config, param: &Param, known: &Map<String, Value>, depth: u8) -> Result<Option<Value>> {
    let Some(op) = lookup_for(param.name) else {
        return Ok(None);
    };
    if config.api_key.is_empty() || depth > 2 {
        return Ok(None);
    }

    let mut args = Map::new();
    for needed in op.params.iter().filter(|p| p.required) {
        let value = match known.get(needed.name) {
            Some(value) => Some(value.clone()),
            None => pick_id(config, needed, known, depth + 1)?,
        };
        let Some(value) = value else {
            return Ok(None); // cannot narrow it down; fall back to typing
        };
        args.insert(needed.name.to_string(), value);
    }

    let noun = op.noun();
    let spinner = view::spinner(&format!("Loading {noun}…"));
    let reply = Client::new(config)?.run(op, &args);
    spinner.finish_and_clear();

    let rows: Vec<Value> = match reply {
        Ok(reply) => op.rows(&reply.value).cloned().unwrap_or_default(),
        // Say why. A token without this scope is worth knowing about; silently
        // demanding an id instead is how you end up hunting through the web app.
        Err(error) => {
            view::warn(&format!("Could not list {noun} — {error}"));
            return Ok(None);
        }
    };
    if rows.is_empty() {
        view::warn(&format!("You have no {noun}."));
        return Ok(None);
    }

    let id_field = view::id_field(op.item_schema).unwrap_or("id");
    let labels: Vec<String> = rows.iter().map(|row| describe(row, op.item_schema, id_field)).collect();
    let chosen = FuzzySelect::new().with_prompt(param.name).items(&labels).default(0).interact()?;
    Ok(Some(Value::from(ops::scalar(&rows[chosen][id_field]))))
}

/// The row as its own table would show it, falling back to the bare id.
fn describe(row: &Value, schema: &str, id_field: &str) -> String {
    view::label(schema, row).unwrap_or_else(|| ops::scalar(&row[id_field]))
}

/// `merc send` — the one flow worth walking someone through by hand.
pub fn send_money(config: &Config) -> Result<(&'static Op, Map<String, Value>)> {
    let op = ops::find("accounts", "create-transaction").expect("send money operation exists");
    let mut args = Map::new();

    for name in ["accountId", "recipientId"] {
        let param = op.param(name).expect("send money takes an account and a recipient");
        let Some(value) = pick_id(config, param, &args, 0)? else {
            bail!("Could not list {name}s — pass --{name} yourself, or check the token's scopes.");
        };
        args.insert(name.to_string(), value);
    }

    let amount: String = Input::new().with_prompt("Amount in dollars").interact_text()?;
    let amount = Money::parse(&amount)?;
    if amount.cents() <= 0 {
        bail!("An amount has to be positive.");
    }
    args.insert("amount".into(), amount.to_api());

    let methods = op.param("paymentMethod").map(|p| p.choices).unwrap_or_default();
    let chosen = Select::new().with_prompt("How").items(methods).default(0).interact()?;
    args.insert("paymentMethod".into(), Value::from(methods[chosen]));

    let memo: String = Input::new().with_prompt("Memo (optional)").allow_empty(true).interact_text()?;
    if !memo.is_empty() {
        args.insert("externalMemo".into(), Value::from(memo));
    }

    println!("\n  Sending {} by {}", style(amount.display()).bold(), methods[chosen]);
    Ok((op, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_flow_fills_everything_the_api_demands() {
        let op = ops::find("accounts", "create-transaction").unwrap();
        // What send_money() sets by hand, plus the key main() fills in.
        let filled = ["accountId", "recipientId", "amount", "paymentMethod", "idempotencyKey"];
        for param in op.params.iter().filter(|p| p.required) {
            assert!(filled.contains(&param.name), "the send flow never asks for {}", param.name);
        }
    }

    #[test]
    fn nothing_asks_for_an_id_it_cannot_offer_a_list_for() {
        let missing: Vec<&str> = ops::OPS
            .iter()
            .flat_map(|op| op.params)
            .filter(|param| param.name.ends_with("Id") && param.loc == ops::In::Path)
            .map(|param| param.name)
            .filter(|name| lookup_for(name).is_none())
            .collect();
        assert!(missing.is_empty(), "no listing behind: {missing:?} — add a row to LOOKUPS");
    }

    #[test]
    fn every_lookup_points_at_a_listing_that_can_label_its_rows() {
        for (param, group, verb) in LOOKUPS {
            let op = ops::find(group, verb).unwrap_or_else(|| panic!("{param}: no `merc {group} {verb}`"));
            assert!(op.list_key.is_some(), "{param}: `merc {group} {verb}` is not a list");
            assert!(
                view::id_field(op.item_schema).is_some(),
                "{param}: nothing tells us which field of a {} is its id",
                op.item_schema
            );
            assert!(
                view::label(op.item_schema, &serde_json::json!({})).is_none(),
                "{param}: an empty row should not produce a label"
            );
        }
    }

    #[test]
    fn a_listing_that_needs_its_own_id_can_reach_one() {
        // Statements belong to an account, so choosing a statement has to offer
        // the accounts first — otherwise `merc statements get-pdf` is a dead end.
        let statements = lookup_for("statementId").expect("statementId is looked up");
        for needed in statements.params.iter().filter(|p| p.required) {
            assert!(lookup_for(needed.name).is_some(), "{} cannot be resolved", needed.name);
        }
    }

    #[test]
    fn a_row_is_offered_by_what_it_is_not_by_its_id() {
        let account = serde_json::json!({
            "id": "acc_1", "name": "Operating", "type": "checking", "currentBalance": 1234.5
        });
        assert_eq!(view::label("Account", &account).unwrap(), "Operating  checking  $1,234.50");
        assert_eq!(view::id_field("Account").unwrap(), "id");
        // A card on an account keeps its id somewhere else entirely.
        assert_eq!(view::id_field("AccountCard").unwrap(), "cardId");
    }

    #[test]
    fn a_prompt_is_one_line_however_long_the_docs_are() {
        let limit = ops::find("accounts", "list").unwrap().param("limit").unwrap();
        assert_eq!(hint(limit), "Maximum number of results to return");

        // The worst offender in the whole spec, echoed back by every menu.
        let cursor = ops::find("accounts", "list").unwrap().param("start_after").unwrap();
        assert!(hint(cursor).chars().count() <= 56, "{}", hint(cursor));
        for op in ops::OPS {
            for param in op.params {
                assert!(hint(param).chars().count() <= 56, "{}: {}", param.name, hint(param));
            }
        }
    }

    #[test]
    fn payment_methods_come_from_the_spec() {
        let op = ops::find("accounts", "create-transaction").unwrap();
        let methods = op.param("paymentMethod").unwrap().choices;
        assert!(methods.contains(&"ach"), "got {methods:?}");
    }

    #[test]
    fn account_and_recipient_arguments_are_offered_as_a_list() {
        let op = ops::find("accounts", "create-transaction").unwrap();
        for name in ["accountId", "recipientId"] {
            assert!(op.param(name).is_some(), "{name} is not a parameter any more");
        }
    }

    #[test]
    fn a_row_with_no_layout_falls_back_to_its_id() {
        let row = serde_json::json!({"id": "xyz_1"});
        assert_eq!(describe(&row, "SomethingMercuryAddedLater", "id"), "xyz_1");
    }
}
