//! Printing: a readable version for people, JSON for scripts and agents. Every JSON reply says
//! where it came from and how fresh that is, so an agent reading the copy knows it may be a couple
//! of minutes behind.

use crate::db::{now_ms, Db, Source};
use crate::store::{iso, Chat, Message};
use anyhow::Result;
use chrono::{Datelike, Local, TimeZone};
use console::style;
use serde_json::{json, Value};

pub fn chats(db: &Db, chats: &[Chat], json_out: bool) -> Result<()> {
    if json_out {
        return emit(db, json!({ "chats": chats }));
    }
    freshness(db);
    for chat in chats {
        let unread = match chat.unread {
            0 => style(format!("{:>4}", "")),
            n => style(format!("{n:>4}")).green().bold(),
        };
        let tags: Vec<&str> = [(chat.kind == "group", "group"), (chat.pinned, "pinned"), (chat.archived, "archived")]
            .into_iter()
            .filter_map(|(on, tag)| on.then_some(tag))
            .collect();
        let group = match tags.is_empty() {
            true => String::new(),
            false => style(format!(" · {}", tags.join(" · "))).dim().to_string(),
        };
        println!(
            "{}  {}  {}{}  {}",
            style(format!("{:>12}", short(chat.last_ms))).dim(),
            unread,
            style(&chat.name).bold(),
            group,
            style(preview(&chat.last_text, 60)).dim()
        );
    }
    Ok(())
}

pub fn conversation(db: &Db, chat: &Chat, messages: &[Message], json_out: bool) -> Result<()> {
    if json_out {
        return emit(db, json!({ "chat": chat, "messages": messages }));
    }
    freshness(db);
    println!("{}  {}", style(&chat.name).bold(), style(format!("{} · {} messages", chat.kind, messages.len())).dim());
    println!();
    for message in messages {
        line(message, None);
    }
    Ok(())
}

pub fn unread(db: &Db, unread: &[(Chat, Vec<Message>)], json_out: bool) -> Result<()> {
    if json_out {
        let chats: Vec<Value> = unread
            .iter()
            .map(|(chat, messages)| json!({ "chat": chat, "messages": messages }))
            .collect();
        return emit(db, json!({ "chats": chats }));
    }
    freshness(db);
    if unread.is_empty() {
        println!("Nothing unread.");
        return Ok(());
    }
    for (chat, messages) in unread {
        println!("{}  {}", style(&chat.name).bold(), style(format!("{} unread", chat.unread)).green());
        for message in messages {
            line(message, None);
        }
        println!();
    }
    Ok(())
}

pub fn hits(db: &Db, query: &str, hits: &[Message], json_out: bool) -> Result<()> {
    if json_out {
        return emit(db, json!({ "query": query, "hits": hits }));
    }
    freshness(db);
    if hits.is_empty() {
        println!("Nothing matches \"{query}\".");
        return Ok(());
    }
    for hit in hits {
        line(hit, hit.chat.as_deref());
    }
    Ok(())
}

/// An error, in the same shape as everything else: JSON for an agent, a red line for a person.
pub fn failure(error: &anyhow::Error, json_out: bool) {
    if json_out {
        println!("{}", json!({ "error": format!("{error:#}") }));
        return;
    }
    eprintln!("{} {error:#}", style("✗").red());
}

fn emit(db: &Db, payload: Value) -> Result<()> {
    let mut out = json!({
        "source": match db.source { Source::Live => "live", Source::Snapshot => "snapshot" },
        "as_of": iso(db.as_of),
    });
    if let (Some(out), Value::Object(fields)) = (out.as_object_mut(), payload) {
        out.extend(fields);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn freshness(db: &Db) {
    let note = match db.source {
        Source::Live => "live · straight from WhatsApp".to_string(),
        Source::Snapshot => format!("copy · {} old (refreshed every 2 minutes by the sync job)", ago(now_ms() - db.as_of)),
    };
    println!("{}", style(note).dim());
    println!();
}

/// One message: when, who, and what, with any further lines indented under the first.
fn line(message: &Message, chat: Option<&str>) {
    let who = match (message.mine, chat) {
        (true, Some(chat)) => format!("{} · me", chat),
        (false, Some(chat)) if chat != message.from => format!("{} · {}", chat, message.from),
        _ => message.from.clone(),
    };
    let who = if message.mine { style(who).cyan() } else { style(who).bold() };
    let mut lines = message.text.lines();
    let first = lines.next().unwrap_or("");
    println!("{}  {}  {}", style(format!("{:>12}", short(message.ms))).dim(), who, first);
    for more in lines {
        println!("{:>14}{}", "", more);
    }
}

fn short(ms: i64) -> String {
    let Some(at) = Local.timestamp_millis_opt(ms).single() else { return String::new() };
    let now = Local::now();
    match (at.date_naive() == now.date_naive(), at.year() == now.year()) {
        (true, _) => at.format("%H:%M").to_string(),
        (false, true) => at.format("%b %e %H:%M").to_string(),
        (false, false) => at.format("%Y-%m-%d").to_string(),
    }
}

pub fn ago(ms: i64) -> String {
    let minutes = ms / 60_000;
    match minutes {
        0 => "under a minute".into(),
        1..=89 => format!("{minutes} min"),
        _ => format!("{} h", minutes / 60),
    }
}

fn preview(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.chars().count() > limit {
        true => format!("{}…", flat.chars().take(limit).collect::<String>()),
        false => flat,
    }
}
