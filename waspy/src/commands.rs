//! The four reads, shared by the command line and the menu.

use crate::{db, render, store};
use anyhow::{bail, Result};

pub fn chats(count: usize, json: bool) -> Result<()> {
    let db = db::open()?;
    let chats = store::chats(&db.conn, count)?;
    render::chats(&db, &chats, json)
}

pub fn unread(json: bool) -> Result<()> {
    let db = db::open()?;
    let unread = store::unread(&db.conn)?;
    render::unread(&db, &unread, json)
}

pub fn read(query: &str, count: usize, since: Option<&str>, json: bool) -> Result<()> {
    let db = db::open()?;
    let chat = store::find_chat(&db.conn, query)?;
    let since = since.map(store::parse_since).transpose()?;
    let messages = store::messages(&db.conn, &chat, count, since)?;
    render::conversation(&db, &chat, &messages, json)
}

pub fn search(query: &str, count: usize, chat: Option<&str>, json: bool) -> Result<()> {
    if query.trim().is_empty() {
        bail!("search needs something to look for");
    }
    let db = db::open()?;
    let chat = chat.map(|name| store::find_chat(&db.conn, name)).transpose()?;
    let hits = store::search(&db.conn, query, count, chat.as_ref())?;
    render::hits(&db, query, &hits, json)
}
