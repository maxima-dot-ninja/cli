//! merc — a command line for the whole Mercury API.
//!
//! The commands are not written here. They are generated from `openapi.json` at
//! build time, which is what makes "all of it" a fact rather than a claim. This
//! file is the part that cannot be generated: how a flag becomes a value, what
//! gets confirmed before it runs, and what happens when you type `merc` alone.

mod client;
mod config;
mod money;
mod ops;
mod view;
mod wizard;

use anyhow::{bail, Context, Result};
use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command};
use client::Client;
use config::Config;
use console::style;
use money::Money;
use ops::{In, Op, Param, RAW_BODY};
use serde_json::{Map, Value};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // `{:#}` keeps the whole chain, so "--amount on `merc …`" is followed
            // by the reason it was refused rather than replacing it.
            eprintln!("\n  {} {}\n", style("Error:").red().bold(), indent(&format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}

fn indent(message: &str) -> String {
    message.replace('\n', "\n  ")
}

fn run() -> Result<()> {
    let matches = command().get_matches();

    let mut config = config::load()?;
    if matches.get_flag("sandbox") {
        config.sandbox = true;
    }
    let flags = Flags::read(&matches);

    match matches.subcommand() {
        None => interactive(&config, &flags),
        Some(("config", _)) => show_config(&config),
        Some(("ops", sub)) => {
            catalogue(sub.get_one::<String>("group").map(String::as_str));
            Ok(())
        }
        Some(("send", _)) => {
            wizard::header(&config);
            let (op, args) = wizard::send_money(&config)?;
            execute(&config, &flags, op, args)
        }
        Some(("call", sub)) => call(&config, &flags, sub),
        Some((group, sub)) => group_command(&config, &flags, group, sub),
    }
}

/// Flags that apply to every operation, so they are defined once.
struct Flags {
    json: bool,
    yes: bool,
    all: bool,
    body: Option<String>,
    out: Option<String>,
}

impl Flags {
    fn read(matches: &ArgMatches) -> Self {
        Self {
            json: matches.get_flag("json"),
            yes: matches.get_flag("yes"),
            all: matches.get_flag("all"),
            body: matches.get_one::<String>("body").cloned(),
            out: matches.get_one::<String>("out").cloned(),
        }
    }
}

// ── the command tree ────────────────────────────────────────────────────────

fn command() -> Command {
    let globals = [
        Arg::new("json")
            .long("json")
            .global(true)
            .help_heading("Global")
            .action(ArgAction::SetTrue)
            .help("Print Mercury's reply exactly as it arrived"),
        Arg::new("yes")
            .long("yes")
            .short('y')
            .global(true)
            .help_heading("Global")
            .action(ArgAction::SetTrue)
            .help("Skip the confirmation"),
        Arg::new("all")
            .long("all")
            .global(true)
            .help_heading("Global")
            .action(ArgAction::SetTrue)
            .help("Follow every page of a list"),
        Arg::new("sandbox")
            .long("sandbox")
            .global(true)
            .help_heading("Global")
            .action(ArgAction::SetTrue)
            .help("Use the sandbox, not real money"),
        Arg::new("body")
            .long("body")
            .global(true)
            .help_heading("Global")
            .value_name("JSON|@FILE|-")
            .help("Send this as the request body; flags still override single fields"),
        Arg::new("out")
            .long("out")
            .global(true)
            .help_heading("Global")
            .value_name("PATH")
            .help("Where to write a downloaded file"),
    ];

    let mut root = Command::new("merc")
        .version(env!("CARGO_PKG_VERSION"))
        .about("The Mercury API, all of it, from the command line")
        .after_help(
            "Run `merc` with nothing to choose a command, `merc ops` to list all of them,\n\
             or `merc <group> <command> --help` for one operation's arguments.",
        )
        .args(globals)
        .subcommand(
            Command::new("config").about("Show where the token comes from and which Mercury it reaches"),
        )
        .subcommand(
            Command::new("ops")
                .about("List every operation")
                .arg(Arg::new("group").help("Only this group").value_parser(ops::groups())),
        )
        .subcommand(Command::new("send").about("Send money, step by step"))
        .subcommand(
            Command::new("call")
                .about("Run an operation by its Mercury id: merc call getAccountCards accountId=…")
                .arg(Arg::new("operation").required(true).help("Operation id, as shown by `merc ops`"))
                .arg(Arg::new("args").num_args(0..).trailing_var_arg(true).help("key=value pairs")),
        );

    for group in ops::groups() {
        let commands = ops::in_group(group);
        let mut sub =
            Command::new(group).about(summarise(group, &commands)).subcommand_help_heading("Commands");

        // `merc transactions --limit 5` is what people type, so the listing
        // command's own flags are accepted on the group as well as on `list`.
        for param in ops::default_op(group).map(|op| op.params).unwrap_or_default() {
            sub = sub.arg(argument(param));
        }
        for op in commands {
            sub = sub.subcommand(operation_command(op));
        }
        root = root.subcommand(sub);
    }
    root
}

fn summarise(group: &'static str, commands: &[&'static Op]) -> String {
    let listing = match ops::default_op(group) {
        Some(_) => "lists on its own",
        None => "needs a command",
    };
    let plural = match commands.len() {
        1 => "command",
        _ => "commands",
    };
    format!("{} {plural} — {listing}", commands.len())
}

fn operation_command(op: &'static Op) -> Command {
    let mut command = Command::new(op.verb)
        .about(op.about)
        .after_help(format!("{} {}", op.method.label(), op.path))
        .arg_required_else_help(false);
    if !op.notes.is_empty() {
        command = command.long_about(format!("{}\n\n{} {}", op.notes, op.method.label(), op.path));
    }
    for param in op.params {
        command = command.arg(argument(param));
    }
    command
}

/// One parameter, as a flag. The API's own spelling is the flag name so that
/// anything in Mercury's docs can be typed straight in; the readable spelling is
/// an alias.
fn argument(param: &'static Param) -> Arg {
    // Required is shown, not enforced by clap: `--body` can satisfy body fields,
    // and the check that knows about that lives in finalise().
    let help = match param.required {
        true => format!("{} [required]", param.about).trim().to_string(),
        false => param.about.to_string(),
    };
    let mut arg = Arg::new(param.name).long(param.name).help(help).value_name(param.hint());
    if let Some(alias) = param.alias() {
        arg = arg.visible_alias(alias);
    }
    if let Some(letter) = shorthand(param) {
        arg = arg.short(letter);
    }
    match param.ty {
        // Repeatable filters: --status active --status frozen, or --status active,frozen
        "array" => arg = arg.action(ArgAction::Append),
        "boolean" => arg = arg.num_args(0..=1).default_missing_value("true"),
        _ => {}
    }
    if !param.choices.is_empty() && param.ty != "array" {
        arg = arg.value_parser(PossibleValuesParser::new(param.choices));
    }
    arg
}

/// A few flags are typed often enough to earn a letter.
fn shorthand(param: &'static Param) -> Option<char> {
    match param.name {
        "limit" => Some('n'),
        "search" => Some('q'),
        "file" => Some('f'),
        _ => None,
    }
}

// ── running one ─────────────────────────────────────────────────────────────

fn group_command(config: &Config, flags: &Flags, group: &str, matches: &ArgMatches) -> Result<()> {
    let Some((verb, sub)) = matches.subcommand() else {
        // `merc accounts` on its own does the obvious thing.
        let Some(op) = ops::default_op(group) else {
            command().find_subcommand_mut(group).expect("group exists").print_help()?;
            return Ok(());
        };
        return execute(config, flags, op, collect(op, matches)?);
    };

    let op = ops::find(group, verb).expect("clap only accepts generated verbs");
    execute(config, flags, op, collect(op, sub)?)
}

/// Arguments as clap parsed them, turned into the JSON the API expects.
fn collect(op: &'static Op, matches: &ArgMatches) -> Result<Map<String, Value>> {
    let mut args = Map::new();
    for param in op.params {
        let Some(raw) = matches.get_many::<String>(param.name) else {
            continue;
        };
        let values: Vec<&String> = raw.collect();
        let value = match param.is_list() {
            true => Value::Array(
                values
                    .iter()
                    .flat_map(|value| value.split(','))
                    .map(|item| parse_value(op, param, item.trim()))
                    .collect::<Result<Vec<Value>>>()?,
            ),
            false => parse_value(op, param, values[0])?,
        };
        args.insert(param.name.to_string(), value);
    }
    Ok(args)
}

/// A string from the command line becomes the type the parameter is declared as.
///
/// Amounts go through `Money`, which is why `--amount 1,234.56`, `--amount 5k`
/// and `--amount $10` all work and `--amount 1.005` is refused rather than
/// quietly rounded into a payment nobody meant to make.
pub fn parse_value(op: &'static Op, param: &Param, raw: &str) -> Result<Value> {
    let named = || format!("--{} on `merc {}`", param.name, op.command());

    // One check covers both an enum and a repeatable list of them, because a
    // list's values arrive here one at a time.
    if !param.choices.is_empty() && !param.choices.contains(&raw) {
        bail!("{} must be one of: {}", named(), param.choices.join(", "));
    }

    match param.ty {
        "number" => Ok(Money::parse(raw).with_context(named)?.to_api()),
        "integer" => Ok(Value::from(raw.parse::<i64>().with_context(named)?)),
        "boolean" => Ok(Value::from(raw.parse::<bool>().with_context(named)?)),
        "object" | "array" => serde_json::from_str(raw).or_else(|_| match param.ty {
            // A bare word in a list of strings is a string, not broken JSON.
            "array" => Ok(Value::from(raw)),
            _ => bail!("{} needs JSON, got: {raw}", named()),
        }),
        "file" => match std::path::Path::new(raw).is_file() {
            true => Ok(Value::from(raw)),
            false => bail!("{}: no file at {raw}", named()),
        },
        _ => Ok(Value::from(raw)),
    }
}

/// `merc call <operationId> key=value …` — the same machinery, addressed the way
/// Mercury's own documentation addresses it.
fn call(config: &Config, flags: &Flags, matches: &ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("operation").expect("required");
    let Some(op) = ops::find_by_id(name) else {
        bail!("No operation called `{name}`. Run `merc ops` to see them all.");
    };

    let mut args = Map::new();
    for pair in matches.get_many::<String>("args").unwrap_or_default() {
        let Some((key, raw)) = pair.split_once('=') else {
            bail!("`{pair}` is not key=value");
        };
        let Some(param) = op.param(key) else {
            bail!("`{}` has no `{key}`. It takes: {}", op.command(), names(op));
        };
        args.insert(param.name.to_string(), parse_value(op, param, raw)?);
    }
    execute(config, flags, op, args)
}

fn names(op: &'static Op) -> String {
    match op.params.is_empty() {
        true => "no arguments".to_string(),
        false => op.params.iter().map(|p| p.name).collect::<Vec<_>>().join(", "),
    }
}

fn execute(config: &Config, flags: &Flags, op: &'static Op, args: Map<String, Value>) -> Result<()> {
    let args = finalise(config, op, args, flags)?;
    let client = Client::new(config)?;

    if op.mutates() && !flags.yes && !wizard::confirm(config, op, &args, &client::body(op, &args))? {
        println!("  Cancelled.\n");
        return Ok(());
    }

    let spinner = view::spinner(op.about);
    let result = match (client::is_download(op), flags.all) {
        (true, _) => return finish_download(&client, op, &args, flags, spinner),
        (false, true) => client.run_all(op, &args),
        (false, false) => client.run(op, &args),
    };
    spinner.finish_and_clear();

    let reply = result?;
    if reply.was_duplicate {
        view::warn("Mercury already had this idempotency key. Nothing new was sent — this is the original.");
    }
    view::print(op, &reply.value, &reply.raw, flags.json);
    Ok(())
}

/// Everything that has to be true before a request goes out.
fn finalise(
    config: &Config,
    op: &'static Op,
    mut args: Map<String, Value>,
    flags: &Flags,
) -> Result<Map<String, Value>> {
    if let Some(source) = &flags.body {
        let raw = read_body(source)?;
        let parsed: Value = serde_json::from_str(&raw).context("--body is not valid JSON")?;
        args.insert(RAW_BODY.to_string(), parsed);
    }

    // Retrying a payment must not send it twice, so the key is filled in and
    // shown rather than left to whatever the caller remembers to pass.
    if op.param("idempotencyKey").is_some_and(|p| p.required) && !args.contains_key("idempotencyKey") {
        args.insert("idempotencyKey".into(), Value::from(uuid::Uuid::new_v4().to_string()));
    }

    let unknown = op.unknown(&args);
    if !unknown.is_empty() {
        bail!("`merc {}` has no {}. It takes: {}", op.command(), unknown.join(", "), names(op));
    }

    // An id you do not have is not a reason to send you to the web app: if
    // there is someone at the keyboard, offer the list instead.
    if console::user_attended() && !op.missing(&args).is_empty() {
        wizard::fill_missing(config, op, &mut args)?;
    }

    // A hand-written body is the caller's business; a missing path segment is not.
    let missing: Vec<&str> = op
        .missing(&args)
        .iter()
        .filter(|param| param.loc != In::Body || !args.contains_key(RAW_BODY))
        .map(|param| param.name)
        .collect();
    if !missing.is_empty() {
        bail!("`merc {}` needs --{}", op.command(), missing.join(" --"));
    }
    Ok(args)
}

fn read_body(source: &str) -> Result<String> {
    match source {
        "-" => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
            Ok(text)
        }
        path if path.starts_with('@') => {
            std::fs::read_to_string(&path[1..]).with_context(|| format!("Could not read {}", &path[1..]))
        }
        literal => Ok(literal.to_string()),
    }
}

fn finish_download(
    client: &Client,
    op: &'static Op,
    args: &Map<String, Value>,
    flags: &Flags,
    spinner: indicatif::ProgressBar,
) -> Result<()> {
    let file = client.download(op, args);
    spinner.finish_and_clear();
    let file = file?;

    let path = flags.out.clone().unwrap_or(file.filename);
    std::fs::write(&path, &file.bytes).with_context(|| format!("Could not write {path}"))?;
    view::success(&format!("Saved {path} ({} KB)\n", file.bytes.len() / 1024));
    Ok(())
}

// ── the standalone commands ─────────────────────────────────────────────────

fn interactive(config: &Config, flags: &Flags) -> Result<()> {
    wizard::header(config);

    let config = match config.api_key.is_empty() {
        false => config.clone(),
        true => match wizard::setup(config)? {
            Some(ready) => ready,
            None => return Ok(()),
        },
    };

    let Some((op, args)) = wizard::browse(&config)? else {
        return Ok(());
    };
    execute(&config, flags, op, args)
}

fn show_config(config: &Config) -> Result<()> {
    wizard::header(config);
    println!("  Config file : {}", config::config_path()?.display());
    println!("  Environment : {}", config.environment());
    println!("  Base URL    : {}", config.base_url());
    println!(
        "  API token   : {}",
        match config.api_key.is_empty() {
            true => "not set".to_string(),
            false => format!("set ({} chars)", config.api_key.len()),
        }
    );
    println!("  Operations  : {}\n", ops::OPS.len());
    if config.api_key.is_empty() {
        println!("{}\n", config::missing_key_help());
    }
    Ok(())
}

fn catalogue(only: Option<&str>) {
    println!();
    for group in ops::groups() {
        if only.is_some_and(|wanted| wanted != group) {
            continue;
        }
        println!("  {}", style(group).cyan().bold());
        for op in ops::in_group(group) {
            let mark = match op.mutates() {
                true => style("•").yellow().to_string(),
                false => " ".to_string(),
            };
            println!("  {mark} {:<26} {}", op.verb, style(op.about).dim());
        }
        println!();
    }
    println!("  {} operations. {} changes data.\n", ops::OPS.len(), style("•").yellow());
}

#[cfg(test)]
mod tests {
    use super::*;
    use ops::find;

    #[test]
    fn the_command_tree_builds_and_covers_every_operation() {
        let mut root = command();
        root.build();
        for op in ops::OPS {
            let group = root.find_subcommand(op.group).unwrap_or_else(|| panic!("no group {}", op.group));
            assert!(group.find_subcommand(op.verb).is_some(), "no `merc {}`", op.command());
        }
    }

    #[test]
    fn a_group_takes_its_listing_flags_directly() {
        let matches = command().try_get_matches_from(["merc", "transactions", "--limit", "5"]).unwrap();
        let (group, sub) = matches.subcommand().unwrap();
        assert_eq!(group, "transactions");
        assert!(sub.subcommand().is_none(), "no verb was given");

        let op = ops::default_op(group).unwrap();
        assert_eq!(collect(op, sub).unwrap()["limit"], Value::from(5));
    }

    #[test]
    fn flags_and_operations_never_shadow_each_other() {
        // Every reserved word has to stay clear of the generated group names.
        for reserved in ["config", "ops", "send", "call"] {
            assert!(!ops::groups().contains(&reserved), "`merc {reserved}` collides with a group");
        }
        for op in ops::OPS {
            for param in op.params {
                assert!(
                    !["json", "yes", "all", "sandbox", "body", "out", "help", "version"]
                        .contains(&param.name),
                    "{} has a parameter named {} which is also a global flag",
                    op.id,
                    param.name
                );
            }
        }
    }

    #[test]
    fn parsing_an_amount_goes_through_money() {
        let op = find("accounts", "create-transaction").unwrap();
        let amount = op.param("amount").unwrap();
        assert_eq!(parse_value(op, amount, "1,234.56").unwrap().to_string(), "1234.56");
        assert_eq!(parse_value(op, amount, "5k").unwrap().to_string(), "5000.00");
        assert!(parse_value(op, amount, "1.005").is_err(), "a third decimal must not be rounded away");
        assert!(parse_value(op, amount, "ten").is_err());
    }

    #[test]
    fn typed_values_land_as_the_right_json_type() {
        let categories = find("categories", "create").unwrap();
        let boolean = categories.param("visibleForCardSpend").unwrap();
        assert_eq!(parse_value(categories, boolean, "true").unwrap(), Value::Bool(true));
        assert!(parse_value(categories, boolean, "yes").is_err());

        let list = find("cards", "list").unwrap();
        let status = list.param("status").unwrap();
        assert_eq!(parse_value(list, status, "active").unwrap(), Value::from("active"));

        let limit = list.param("limit").unwrap();
        assert_eq!(parse_value(list, limit, "25").unwrap(), Value::from(25));
        assert!(parse_value(list, limit, "lots").is_err());
    }

    #[test]
    fn an_unknown_argument_is_refused_before_anything_is_sent() {
        let op = find("accounts", "list").unwrap();
        let flags = Flags { json: false, yes: true, all: false, body: None, out: None };
        let mut args = Map::new();
        args.insert("limitt".into(), Value::from(5));
        let error = finalise(&Config::default(), op, args, &flags).unwrap_err().to_string();
        assert!(error.contains("has no limitt"), "got: {error}");
    }

    #[test]
    fn a_send_gets_an_idempotency_key_whether_or_not_you_pass_one() {
        let op = find("accounts", "create-transaction").unwrap();
        let flags = Flags { json: false, yes: true, all: false, body: None, out: None };
        let mut args = Map::new();
        for (key, value) in [("accountId", "a"), ("recipientId", "r"), ("paymentMethod", "ach")] {
            args.insert(key.into(), Value::from(value));
        }
        args.insert("amount".into(), Money::parse("10.20").unwrap().to_api());

        let filled = finalise(&Config::default(), op, args.clone(), &flags).unwrap();
        assert!(filled["idempotencyKey"].as_str().is_some_and(|key| key.len() > 30));

        args.insert("idempotencyKey".into(), Value::from("payroll-august"));
        let kept = finalise(&Config::default(), op, args, &flags).unwrap();
        assert_eq!(kept["idempotencyKey"], "payroll-august", "a key that was given must be kept");
    }

    #[test]
    fn a_hand_written_body_excuses_the_fields_it_contains() {
        let op = find("accounts", "create-transaction").unwrap();
        let body = Flags {
            json: false,
            yes: true,
            all: false,
            body: Some(r#"{"recipientId":"r","amount":10.20,"paymentMethod":"ach"}"#.into()),
            out: None,
        };
        let mut args = Map::new();
        args.insert("accountId".into(), Value::from("a"));
        assert!(
            finalise(&Config::default(), op, args.clone(), &body).is_ok(),
            "--body should satisfy the body fields"
        );

        // The path is still not optional.
        let plain = Flags { json: false, yes: true, all: false, body: None, out: None };
        let error = finalise(&Config::default(), op, Map::new(), &plain).unwrap_err().to_string();
        assert!(error.contains("--accountId"), "got: {error}");
    }
}
