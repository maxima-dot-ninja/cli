//! Bare `waspy`: a menu. ↑/↓ and Enter, or a digit to jump straight to an entry; Esc or q leaves.
//! It comes back after each entry, so you can read a chat and then search without starting over.

use crate::{ask, commands, db, render, store, sync};
use anyhow::Result;
use console::{style, Key, Term};
use dialoguer::{FuzzySelect, Input};

type Action = fn() -> Result<()>;

const ENTRIES: &[(&str, Action)] = &[
    ("Recent chats", recent),
    ("Unread", unread),
    ("Read a chat", read),
    ("Search", search),
    ("Ask Claude", ask),
    ("Sync now", sync_now),
    ("Status", sync::status),
];

pub fn interactive() -> bool {
    Term::stdout().is_term() && Term::stderr().is_term()
}

pub fn run() -> Result<()> {
    if !interactive() {
        println!("waspy's menu needs a terminal. `waspy --help` lists the commands.");
        return Ok(());
    }
    loop {
        let Some(index) = pick("waspy · WhatsApp, read-only")? else { return Ok(()) };
        if let Err(error) = (ENTRIES[index].1)() {
            render::failure(&error, false);
        }
        println!();
    }
}

fn recent() -> Result<()> {
    commands::chats(30, false)
}

fn unread() -> Result<()> {
    commands::unread(false)
}

fn sync_now() -> Result<()> {
    sync::run(false)
}

fn read() -> Result<()> {
    let db = db::open()?;
    let chats = store::chats(&db.conn, 300)?;
    let names: Vec<&str> = chats.iter().map(|chat| chat.name.as_str()).collect();
    let picked = FuzzySelect::new().with_prompt("Chat (type to filter)").items(&names).default(0).interact_opt()?;
    let Some(index) = picked else { return Ok(()) };
    let messages = store::messages(&db.conn, &chats[index], 50, None)?;
    render::conversation(&db, &chats[index], &messages, false)
}

fn search() -> Result<()> {
    let query: String = Input::new().with_prompt("Search for").allow_empty(true).interact_text()?;
    if query.trim().is_empty() {
        return Ok(());
    }
    commands::search(&query, 20, None, false)
}

fn ask() -> Result<()> {
    let question: String = Input::new().with_prompt("Ask").allow_empty(true).interact_text()?;
    if question.trim().is_empty() {
        return Ok(());
    }
    ask::run(&question, ask::MODEL, false)
}

/// The menu itself. Drawn on stderr so piping `waspy` never captures it.
fn pick(title: &str) -> Result<Option<usize>> {
    let term = Term::stderr();
    let count = ENTRIES.len();
    let mut index = 0;
    term.hide_cursor()?;
    let chosen = loop {
        draw(&term, title, index)?;
        let key = term.read_key()?;
        term.clear_last_lines(count + 1)?;
        match key {
            Key::ArrowUp | Key::Char('k') => index = (index + count - 1) % count,
            Key::ArrowDown | Key::Char('j') => index = (index + 1) % count,
            Key::Enter => break Some(index),
            Key::Escape | Key::Char('q') | Key::CtrlC => break None,
            Key::Char(c) if digit(c, count).is_some() => break digit(c, count),
            _ => {}
        }
    };
    term.show_cursor()?;
    Ok(chosen)
}

fn digit(c: char, count: usize) -> Option<usize> {
    let n = c.to_digit(10)? as usize;
    (1..=count).contains(&n).then(|| n - 1)
}

fn draw(term: &Term, title: &str, index: usize) -> Result<()> {
    term.write_line(&style(title).bold().to_string())?;
    for (i, (label, _)) in ENTRIES.iter().enumerate() {
        let line = format!("{} {}  {label}", if i == index { "❯" } else { " " }, i + 1);
        let line = if i == index { style(line).cyan().to_string() } else { line };
        term.write_line(&line)?;
    }
    Ok(())
}
