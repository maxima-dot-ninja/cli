//! Where the messages come from.
//!
//! The WhatsApp desktop app keeps every chat in a plain SQLite database inside its own container.
//! macOS lets a terminal read it, but refuses a background process such as vaulty's daemon. So a
//! launchd job — `waspy sync`, the one binary granted Full Disk Access — copies it into waspy's
//! own folder every two minutes, and anything that cannot read the original reads the copy.

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Live,
    Snapshot,
}

pub struct Db {
    pub conn: Connection,
    pub source: Source,
    /// When this data was current, in milliseconds: now for the live database, the copy's age
    /// otherwise.
    pub as_of: i64,
}

/// The desktop app's database.
pub fn live_path() -> PathBuf {
    home().join("Library/Group Containers/group.net.whatsapp.WhatsApp.shared/ChatStorage.sqlite")
}

/// waspy's own folder: the copy, and the record of the last sync.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"));
    base.join("waspy")
}

pub fn snapshot_path() -> PathBuf {
    data_dir().join("ChatStorage.sqlite")
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Whether a person is at a terminal. Only then is WhatsApp's own database tried. In the
/// background, touching another app's data without Full Disk Access makes macOS ask "would like to
/// access data from other apps" — and for a command-line tool its "Allow" is not remembered, so the
/// question comes back on every run. Background readers (vaulty, scripts) use the copy instead.
pub fn interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() || std::io::stdout().is_terminal() || std::io::stderr().is_terminal()
}

/// Full Disk Access is the grant macOS does remember for a command-line tool. The privacy database
/// can only be opened with it, and a refusal there is silent, so trying is a safe test that never
/// puts up a dialog.
pub fn has_full_disk_access() -> bool {
    std::fs::File::open(home().join("Library/Application Support/com.apple.TCC/TCC.db")).is_ok()
}

/// Set by `waspy ask` for the waspy commands Claude runs on the person's behalf, and only when the
/// person asked from a terminal. Those commands have no terminal of their own, but they run inside the
/// person's terminal session, so they may read the live database the way the person can.
pub const LIVE_ENV: &str = "WASPY_LIVE";

/// The live database for a person at a terminal who may read it, the copy otherwise.
pub fn open() -> Result<Db> {
    let may_read_live = interactive() || std::env::var_os(LIVE_ENV).is_some();
    if let Some(conn) = may_read_live.then(open_live).flatten() {
        return Ok(Db { conn, source: Source::Live, as_of: now_ms() });
    }
    let path = snapshot_path();
    let as_of = modified_ms(&path).context(
        "WhatsApp's database is not readable from here, and there is no copy yet. Run `waspy sync` \
         from a terminal once, or `bin/install` to keep one fresh",
    )?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    Ok(Db { conn, source: Source::Snapshot, as_of })
}

/// The original, if macOS lets this process read it. Opening can succeed and the first read still
/// be refused, so a query proves it.
pub fn open_live() -> Option<Connection> {
    let conn = Connection::open_with_flags(live_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    conn.query_row("SELECT count(*) FROM ZWACHATSESSION", [], |row| row.get::<_, i64>(0)).ok()?;
    Some(conn)
}

pub fn modified_ms(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64)
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
