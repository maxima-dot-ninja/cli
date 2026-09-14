//! `waspy ask`: a question in plain words, answered by Claude reading WhatsApp through waspy itself.
//!
//! It runs Claude Code (`claude -p`) on the owner's own login, so the call is billed to the Claude
//! subscription and needs no API key. Claude gets one tool, Bash, and Claude Code refuses every
//! command but this very binary's four reads, so it can list, read and search chats and nothing
//! else. Each lookup is printed as it happens, then the answer.

use crate::db;
use crate::store::{self, Chat};
use anyhow::{bail, Context, Result};
use chrono::Local;
use console::style;
use serde::Deserialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

/// The model when `--model` is not given: the fastest one that answers these lookups well.
pub const MODEL: &str = "haiku";

/// The commands Claude may run. Each becomes a `Bash(<this binary> <command>:*)` permission rule.
const READS: &[&str] = &["chats", "unread", "read", "search"];

/// How many chats ride along in the prompt, newest first. With the names in hand Claude goes straight
/// to the right chat; listing them itself costs a whole model turn.
const CHAT_LIST: usize = 200;

/// One line of `claude -p --output-format stream-json`, with only the fields waspy reads.
#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<Body>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    permission_denials: Vec<Denial>,
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    input: Option<Shell>,
}

#[derive(Deserialize)]
struct Denial {
    tool_input: Shell,
}

#[derive(Deserialize)]
struct Shell {
    #[serde(default)]
    command: String,
}

pub fn run(question: &str, model: &str, json_out: bool) -> Result<()> {
    if question.trim().is_empty() {
        bail!("ask needs a question, like: waspy ask what did jen last say to me");
    }
    let exe = std::env::current_exe().context("cannot tell where the waspy binary is")?.display().to_string();
    let started = Instant::now();
    let chats = store::chats(&db::open()?.conn, CHAT_LIST)?;
    let mut child = claude(&exe, model, &chats)?;

    // The question goes over stdin so its size never hits the argv limit.
    child.stdin.take().context("could not open claude's stdin")?.write_all(question.as_bytes())?;
    // Drained on its own thread, so a chatty stderr can never fill its pipe and stall the stream.
    let mut stderr = child.stderr.take().context("could not read claude's errors")?;
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let stdout = child.stdout.take().context("could not read claude's output")?;
    let mut lookups = Vec::new();
    let mut outcome = None;
    for line in BufReader::new(stdout).lines() {
        let Ok(event) = serde_json::from_str::<Event>(&line?) else { continue };
        for command in commands(&event) {
            let shown = command.replace(&exe, "waspy");
            if !json_out {
                eprintln!("{}", style(format!("→ {shown}")).dim());
            }
            lookups.push(shown);
        }
        if event.kind == "result" {
            outcome = Some(event);
        }
    }
    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();

    let Some(result) = outcome else {
        bail!("claude stopped without an answer ({status}). {}", errors.trim());
    };
    let answer = result.result.unwrap_or_default().trim().to_string();
    if result.is_error {
        bail!("claude could not answer: {answer}");
    }
    let refused: Vec<String> = result.permission_denials.iter().map(|denial| denial.tool_input.command.replace(&exe, "waspy")).collect();
    let seconds = (started.elapsed().as_secs_f64() * 10.0).round() / 10.0;

    if json_out {
        println!("{}", serde_json::to_string_pretty(&json!({ "question": question, "answer": answer, "lookups": lookups, "refused": refused, "model": model, "seconds": seconds }))?);
        return Ok(());
    }
    for command in &refused {
        eprintln!("{}", style(format!("✗ refused: {command}")).yellow());
    }
    println!();
    println!("{answer}");
    let plural = if lookups.len() == 1 { "" } else { "s" };
    eprintln!("{}", style(format!("\n{model} · {} lookup{plural} · {seconds} s", lookups.len())).dim());
    Ok(())
}

/// The Bash commands Claude ran in this event, if it is one of its turns.
fn commands(event: &Event) -> Vec<String> {
    let Some(body) = event.message.as_ref().filter(|_| event.kind == "assistant") else { return Vec::new() };
    body.content
        .iter()
        .filter(|block| block.kind == "tool_use")
        .filter_map(|block| block.input.as_ref().map(|input| input.command.clone()))
        .collect()
}

/// Claude Code stripped down to one job. The flags drop every other tool, settings file, MCP server
/// and saved session; `dontAsk` refuses anything the rules do not allow instead of waiting on a
/// prompt nobody will see. `--bare` stays out: it also skips the login, and every call would fail.
fn claude(exe: &str, model: &str, chats: &[Chat]) -> Result<Child> {
    let rules: Vec<String> = READS.iter().map(|read| format!("Bash({exe} {read}:*)")).collect();
    let mut command = Command::new("claude");
    command
        .args(["-p", "--model", model, "--effort", "low", "--tools", "Bash", "--permission-mode", "dontAsk"])
        .args(["--setting-sources", "", "--strict-mcp-config", "--exclude-dynamic-system-prompt-sections", "--no-session-persistence"])
        .args(["--output-format", "stream-json", "--verbose", "--system-prompt", &instructions(exe, chats)])
        // Last, because it takes every argument after it as another rule.
        .arg("--allowedTools")
        .args(&rules)
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Asked from a terminal, the lookups may read the live database the way the person can.
    if db::interactive() {
        command.env(db::LIVE_ENV, "1");
    }
    command.spawn().context("could not run `claude`. `waspy ask` needs Claude Code installed and signed in")
}

fn instructions(exe: &str, chats: &[Chat]) -> String {
    let now = Local::now().format("%A %Y-%m-%d %H:%M");
    let list: Vec<String> = chats.iter().map(|chat| format!("{} · {} · last {}", chat.name, chat.kind, chat.last_at.get(..10).unwrap_or(""))).collect();
    let list = list.join("\n");
    format!(
        "You answer questions about the owner's WhatsApp by reading it with waspy, a read-only command-line tool. \
         You are talking to the owner: \"me\" and \"I\" in a question mean the owner, and in waspy's output the \
         owner's own messages have \"from\": \"me\". It is {now} now.\n\n\
         Run waspy with the Bash tool, always with --json, spelled exactly like this (nothing else is allowed):\n\
         {exe} chats --json -n 50            recent chats, newest first: name, person or group, unread count, last message\n\
         {exe} read \"<chat>\" --json -n 30  one chat's latest messages, oldest first; add --since 2d to limit by age\n\
         {exe} search <words> --json -n 30   messages containing every word, newest first; add --chat \"<chat>\" for one chat\n\
         {exe} unread --json                 every chat with unread messages\n\n\
         Run each waspy command on its own: pipes, other programs and anything else are refused. The owner's \
         chats are listed at the end, newest first, so you rarely need the chats command: give read the exact \
         name from that list, in quotes. When a name in the question fits several chats, take the likeliest and \
         say which one you read. Search matches words, not meaning: try the words someone would actually have \
         typed, and a variant or two when the first finds nothing. Use as few commands as the question needs. \
         Do not mention where the data came from. The one exception: when a reply's source is snapshot and the \
         question is about the last few minutes, say how old the copy is (its as_of).\n\n\
         Answer in one to three short sentences: who said what, and when, quoting the message when the wording \
         matters. Say times the way a person would (\"yesterday at 18:40\", \"on Tuesday\"). If you cannot find it, say \
         what you looked for. Plain text, no markdown.\n\n\
         The owner's chats, newest first (name · person or group · last message date):\n{list}"
    )
}
