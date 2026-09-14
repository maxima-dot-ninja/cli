//! Reading WhatsApp's own tables. They are Core Data's (the Z-prefixed names), and times are
//! seconds since 2001-01-01, Apple's reference date.

use anyhow::{anyhow, bail, Result};
use chrono::{Local, SecondsFormat, TimeZone};
use rusqlite::Connection;
use serde::Serialize;

/// Apple's reference date, in Unix seconds.
const APPLE_EPOCH: f64 = 978_307_200.0;

/// People and groups. Session type 2 is a broadcast list and 3 is status updates: not chats.
const CHAT_KINDS: &[(i64, &str)] = &[(0, "person"), (1, "group")];

/// Message types with no text of their own, and what to call them. Anything not listed and with
/// no text (reactions, call records, security-code notices) has nothing to read and is left out.
const MEDIA: &[(i64, &str)] = &[
    (1, "photo"),
    (2, "video"),
    (3, "voice note"),
    (4, "contact"),
    (5, "location"),
    (8, "document"),
    (11, "GIF"),
    (15, "sticker"),
];

/// WhatsApp keeps a pinned chat on top by pushing its sort date thousands of years ahead (5,000,
/// 6,000 and 7,000 years, one per pin). Anything more than a century ahead is a pin.
const PINNED_GAP: f64 = 100.0 * 31_557_600.0;

/// Message types whose text is not for reading. A group update, a system notice and a deleted
/// message all carry a member's raw id where the text would be.
const SYSTEM: &[(i64, &str)] = &[(6, "group update"), (10, "system notice"), (14, "deleted message")];

/// How long `--since` units are, in milliseconds.
const UNITS: &[(char, i64)] = &[('m', 60_000), ('h', 3_600_000), ('d', 86_400_000), ('w', 604_800_000)];

/// Who sent a message: the saved contact name, then their first name, then the name they gave
/// WhatsApp, then their number.
const SENDER: &str = "COALESCE(NULLIF(gm.ZCONTACTNAME, ''), NULLIF(gm.ZFIRSTNAME, ''), NULLIF(pn.ZPUSHNAME, ''), \
                      NULLIF(m.ZPUSHNAME, ''), gm.ZMEMBERJID, m.ZFROMJID, '')";

const FIELDS: &str = "COALESCE(m.ZMESSAGEDATE, 0), COALESCE(m.ZISFROMME, 0), COALESCE(m.ZTEXT, ''), \
                      COALESCE(m.ZMESSAGETYPE, 0)";

const JOINS: &str = "LEFT JOIN ZWAGROUPMEMBER gm ON gm.Z_PK = m.ZGROUPMEMBER \
                     LEFT JOIN ZWAPROFILEPUSHNAME pn ON pn.ZJID = gm.ZMEMBERJID \
                     LEFT JOIN ZWAMEDIAITEM mi ON mi.Z_PK = m.ZMEDIAITEM";

#[derive(Debug, Clone, Serialize)]
pub struct Chat {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub unread: i64,
    pub archived: bool,
    pub pinned: bool,
    pub last_at: String,
    pub last_text: String,
    #[serde(skip)]
    pub key: i64,
    #[serde(skip)]
    pub last_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub at: String,
    pub from: String,
    pub text: String,
    /// Set on search hits, which come from many chats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(skip)]
    pub ms: i64,
    #[serde(skip)]
    pub mine: bool,
}

/// One message row as it comes out of the database.
struct Raw {
    secs: f64,
    mine: bool,
    text: String,
    kind: i64,
    sender: String,
    title: String,
}

impl Raw {
    fn read(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Raw {
            secs: row.get(0)?,
            mine: row.get::<_, i64>(1)? != 0,
            text: row.get(2)?,
            kind: row.get(3)?,
            sender: row.get(4)?,
            title: row.get(5)?,
        })
    }

    fn into_message(self, chat_name: &str, chat_kind: &str, label: Option<String>) -> Option<Message> {
        let text = body(&self.text, self.kind, &self.title)?;
        let ms = to_ms(self.secs);
        let from = speaker(self.mine, chat_kind, chat_name, &self.sender);
        Some(Message { at: iso(ms), from, text, chat: label, ms, mine: self.mine })
    }
}

/// Chats, most recent first. The date and preview come from each chat's actual last message:
/// the chat row's own date is pushed millennia ahead for a pinned chat, and its preview field
/// holds encoded data rather than text.
pub fn chats(conn: &Connection, limit: usize) -> Result<Vec<Chat>> {
    let sql = format!(
        "SELECT * FROM (SELECT s.Z_PK, COALESCE(s.ZCONTACTJID, ''), COALESCE(NULLIF(s.ZPARTNERNAME, ''), s.ZCONTACTJID, ''), \
         s.ZSESSIONTYPE, MAX(COALESCE(s.ZUNREADCOUNT, 0), 0), COALESCE(s.ZARCHIVED, 0), \
         COALESCE(lm.ZMESSAGEDATE, (SELECT MAX(x.ZMESSAGEDATE) FROM ZWAMESSAGE x WHERE x.ZCHATSESSION = s.Z_PK)) AS real_last, \
         COALESCE(s.ZLASTMESSAGEDATE, 0), COALESCE(lm.ZTEXT, ''), COALESCE(lm.ZMESSAGETYPE, 0), \
         COALESCE(lmi.ZTITLE, lmi.ZVCARDNAME, ''), COALESCE(lm.ZISFROMME, 0) \
         FROM ZWACHATSESSION s \
         LEFT JOIN ZWAMESSAGE lm ON lm.Z_PK = s.ZLASTMESSAGE \
         LEFT JOIN ZWAMEDIAITEM lmi ON lmi.Z_PK = lm.ZMEDIAITEM \
         WHERE s.ZSESSIONTYPE IN (0, 1) AND COALESCE(s.ZHIDDEN, 0) = 0 AND COALESCE(s.ZREMOVED, 0) = 0) \
         WHERE real_last IS NOT NULL ORDER BY real_last DESC LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let real: f64 = row.get(6)?;
        let sort: f64 = row.get(7)?;
        let said = body(&row.get::<_, String>(8)?, row.get(9)?, &row.get::<_, String>(10)?).unwrap_or_default();
        let mine = row.get::<_, i64>(11)? != 0 && !said.is_empty();
        let last_ms = to_ms(real);
        Ok(Chat {
            key: row.get(0)?,
            id: row.get(1)?,
            name: row.get(2)?,
            kind: kind_of(row.get(3)?),
            unread: row.get(4)?,
            archived: row.get::<_, i64>(5)? != 0,
            pinned: sort - real > PINNED_GAP,
            last_ms,
            last_at: iso(last_ms),
            last_text: if mine { format!("me: {said}") } else { said },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// A chat's messages, oldest first: the newest `limit` of them, or those since `since_ms`.
pub fn messages(conn: &Connection, chat: &Chat, limit: usize, since_ms: Option<i64>) -> Result<Vec<Message>> {
    let since = since_ms.map(|ms| ms as f64 / 1000.0 - APPLE_EPOCH).unwrap_or(f64::MIN);
    let sql = format!(
        "SELECT {FIELDS}, {SENDER}, COALESCE(mi.ZTITLE, mi.ZVCARDNAME, '') \
         FROM ZWAMESSAGE m {JOINS} \
         WHERE m.ZCHATSESSION = ?1 AND m.ZMESSAGEDATE >= ?2 \
         ORDER BY m.ZMESSAGEDATE DESC LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![chat.key, since], Raw::read)?;
    let mut out: Vec<Message> = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|raw| raw.into_message(&chat.name, chat.kind, None))
        .collect();
    out.reverse();
    Ok(out)
}

/// Chats with something unread, each with its unread messages.
pub fn unread(conn: &Connection) -> Result<Vec<(Chat, Vec<Message>)>> {
    let mut out = Vec::new();
    for chat in chats(conn, 1000)?.into_iter().filter(|chat| chat.unread > 0) {
        let limit = (chat.unread as usize).min(50);
        let incoming: Vec<Message> = messages(conn, &chat, limit, None)?.into_iter().filter(|m| !m.mine).collect();
        out.push((chat, incoming));
    }
    Ok(out)
}

/// Messages containing every word of `query`, newest first.
pub fn search(conn: &Connection, query: &str, limit: usize, chat: Option<&Chat>) -> Result<Vec<Message>> {
    let words: Vec<String> = query.split_whitespace().map(like_pattern).collect();
    if words.is_empty() {
        bail!("search needs something to look for");
    }
    let clauses = vec!["m.ZTEXT LIKE ? ESCAPE '\\'"; words.len()].join(" AND ");
    let scope = chat.map(|c| format!(" AND s.Z_PK = {}", c.key)).unwrap_or_default();
    let sql = format!(
        "SELECT {FIELDS}, {SENDER}, COALESCE(mi.ZTITLE, mi.ZVCARDNAME, ''), \
         COALESCE(NULLIF(s.ZPARTNERNAME, ''), s.ZCONTACTJID, ''), s.ZSESSIONTYPE \
         FROM ZWAMESSAGE m JOIN ZWACHATSESSION s ON s.Z_PK = m.ZCHATSESSION {JOINS} \
         WHERE s.ZSESSIONTYPE IN (0, 1) AND {clauses}{scope} \
         ORDER BY m.ZMESSAGEDATE DESC LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(words.iter()), |row| {
        Ok((Raw::read(row)?, row.get::<_, String>(6)?, row.get::<_, i64>(7)?))
    })?;
    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|(raw, name, kind)| raw.into_message(&name, kind_of(kind), Some(name.clone())))
        .collect())
}

/// The chat `query` names: an exact id or name first, then the only name containing it.
pub fn find_chat(conn: &Connection, query: &str) -> Result<Chat> {
    let wanted = query.trim().to_lowercase();
    if wanted.is_empty() {
        bail!("which chat? Give any part of its name");
    }
    let all = chats(conn, 5000)?;
    let exact = all.iter().find(|c| c.id.to_lowercase() == wanted || c.name.to_lowercase() == wanted);
    if let Some(chat) = exact {
        return Ok(chat.clone());
    }
    let close: Vec<&Chat> = all.iter().filter(|c| c.name.to_lowercase().contains(&wanted)).collect();
    match close.as_slice() {
        [one] => Ok((*one).clone()),
        [] => bail!("no chat matches \"{query}\". `waspy chats` lists them"),
        many => {
            let names: Vec<&str> = many.iter().take(8).map(|c| c.name.as_str()).collect();
            bail!("\"{query}\" matches {} chats ({}). Be more specific", many.len(), names.join(", "))
        }
    }
}

/// `30m`, `6h`, `2d` or `1w`, as the moment that long ago.
pub fn parse_since(text: &str) -> Result<i64> {
    let text = text.trim();
    let usage = || anyhow!("--since takes a number and a unit: 30m, 6h, 2d or 1w");
    let unit = text.chars().last().ok_or_else(usage)?;
    let (_, size) = UNITS.iter().find(|(u, _)| *u == unit).ok_or_else(usage)?;
    let count: i64 = text[..text.len() - unit.len_utf8()].parse().map_err(|_| usage())?;
    Ok(crate::db::now_ms() - count * size)
}

fn kind_of(session_type: i64) -> &'static str {
    CHAT_KINDS.iter().find(|(t, _)| *t == session_type).map_or("person", |(_, name)| name)
}

/// What a message says: its text, or a label for media, with any caption or file name after it.
fn body(text: &str, kind: i64, title: &str) -> Option<String> {
    if let Some((_, label)) = SYSTEM.iter().find(|(k, _)| *k == kind) {
        return Some(format!("[{label}]"));
    }
    let label = MEDIA.iter().find(|(k, _)| *k == kind).map(|(_, name)| *name);
    let words = [text, title].into_iter().map(str::trim).find(|s| !s.is_empty()).unwrap_or("");
    match (label, words.is_empty()) {
        (Some(label), true) => Some(format!("[{label}]")),
        (Some(label), false) => Some(format!("[{label}] {words}")),
        (None, false) => Some(words.to_string()),
        (None, true) => None,
    }
}

fn speaker(mine: bool, chat_kind: &str, chat_name: &str, sender: &str) -> String {
    match (mine, chat_kind, sender.is_empty()) {
        (true, _, _) => "me".into(),
        (false, "group", false) => readable_jid(sender),
        (false, "group", true) => "group".into(),
        _ => chat_name.to_string(),
    }
}

/// `15551234567@s.whatsapp.net` reads better as `+15551234567`. Anything else is left as it is.
fn readable_jid(sender: &str) -> String {
    let Some((number, "s.whatsapp.net")) = sender.split_once('@') else {
        return sender.to_string();
    };
    format!("+{number}")
}

fn like_pattern(word: &str) -> String {
    let escaped = word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

fn to_ms(secs: f64) -> i64 {
    ((secs + APPLE_EPOCH) * 1000.0) as i64
}

pub fn iso(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.to_rfc3339_opts(SecondsFormat::Secs, false))
        .unwrap_or_default()
}
