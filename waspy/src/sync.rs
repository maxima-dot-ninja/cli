//! `waspy sync`: copy the desktop app's database into waspy's folder, and `waspy status`.
//!
//! The copy uses SQLite's backup API against the live database opened read-only, so it is a
//! consistent snapshot even while WhatsApp is writing. It is written to a temporary file and
//! renamed into place, so a reader sees the old copy or the new one, never half of one.
//!
//! A copy is skipped when the database has not changed since the last one: rewriting 55 MB every
//! two minutes regardless would be about 40 GB of disk writes a day, mostly overnight copies of
//! nothing. Every attempt is recorded, so a job that has stopped working says so in `waspy status`.

use crate::db;
use crate::render::ago;
use anyhow::{anyhow, Context, Result};
use console::style;
use rusqlite::backup::Backup;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The launchd job `bin/install` sets up.
pub const JOB: &str = "ninja.maxima.waspy.sync";

pub fn run(quiet: bool) -> Result<()> {
    // In the background, WhatsApp's folder is not touched at all without Full Disk Access — not
    // even to look at it — because that is what puts up macOS's dialog, every two minutes.
    if !db::interactive() && !db::has_full_disk_access() {
        let error = anyhow!(
            "needs Full Disk Access to copy WhatsApp in the background. Add ~/.cargo/bin/waspy under \
             System Settings → Privacy & Security → Full Disk Access; the next sync picks it up"
        );
        record(&Err(anyhow!("{error}")), 0, "");
        return Err(error);
    }
    let say = |text: String| {
        if !quiet {
            println!("{text}");
        }
    };
    let current = signature();
    let last = last_record();
    let unchanged = db::snapshot_path().exists()
        && last.as_ref().is_some_and(|entry| entry["ok"].as_bool() == Some(true) && entry["signature"].as_str() == Some(current.as_str()));
    if unchanged {
        mark_checked(last);
        say("✓ nothing new since the last copy, so it was left as it is".to_string());
        return Ok(());
    }

    say(format!("→ reading {}", db::live_path().display()));
    let started = Instant::now();
    let outcome = copy();
    let ms = started.elapsed().as_millis() as u64;
    record(&outcome, ms, &current);
    let bytes = outcome?;
    say(format!(
        "✓ copied {:.1} MB in {ms} ms to {}",
        bytes as f64 / 1_048_576.0,
        db::snapshot_path().display()
    ));
    Ok(())
}

/// The live database's size and last change, and its write-ahead log's. Any new message moves
/// one of them. Empty when the files cannot even be looked at, so it never matches a real one.
fn signature() -> String {
    ["", "-wal"]
        .iter()
        .filter_map(|suffix| {
            let path = PathBuf::from(format!("{}{suffix}", db::live_path().display()));
            let size = std::fs::metadata(&path).ok()?.len();
            Some(format!("{size}:{}", db::modified_ms(&path)?))
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn copy() -> Result<u64> {
    let source = db::open_live().context(
        "macOS will not let this process read WhatsApp's database. Give ~/.cargo/bin/waspy Full Disk \
         Access: System Settings → Privacy & Security → Full Disk Access",
    )?;
    std::fs::create_dir_all(db::data_dir())?;
    let target = db::snapshot_path();
    let temp = target.with_extension(format!("sqlite.{}.part", std::process::id()));
    let written = backup(&source, &temp);
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written?;
    std::fs::rename(&temp, &target)?;
    Ok(std::fs::metadata(&target)?.len())
}

fn backup(source: &Connection, temp: &Path) -> Result<()> {
    let mut dest = Connection::open(temp)?;
    let copy = Backup::new(source, &mut dest)?;
    copy.run_to_completion(4096, Duration::ZERO, None)?;
    Ok(())
}

fn status_file() -> PathBuf {
    db::data_dir().join("sync.json")
}

fn last_record() -> Option<Value> {
    let text = std::fs::read_to_string(status_file()).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_record(entry: &Value) {
    let _ = std::fs::create_dir_all(db::data_dir());
    let _ = std::fs::write(status_file(), entry.to_string());
}

fn record(outcome: &Result<u64>, ms: u64, signature: &str) {
    let now = db::now_ms();
    let entry = match outcome {
        Ok(bytes) => json!({ "at": now, "checked": now, "ok": true, "bytes": bytes, "ms": ms, "signature": signature }),
        Err(error) => json!({ "at": now, "checked": now, "ok": false, "error": format!("{error:#}"), "ms": ms }),
    };
    write_record(&entry);
}

/// A skipped copy still counts as a check, so `waspy status` can tell a quiet chat from a dead job.
fn mark_checked(last: Option<Value>) {
    let Some(mut entry) = last else { return };
    entry["checked"] = json!(db::now_ms());
    write_record(&entry);
}

pub fn status() -> Result<()> {
    // Looking at WhatsApp's folder from the background is exactly what raises the dialog.
    let live = db::interactive() && db::open_live().is_some();
    let snapshot = db::snapshot_path();
    let last = last_record();
    let now = db::now_ms();

    let live_line = match live {
        true => style("readable from here").green().to_string(),
        false => style("not read from here (the copy is used instead)").yellow().to_string(),
    };
    let copy_line = match db::modified_ms(&snapshot) {
        Some(at) => format!("{} old · {}", ago(now - at), snapshot.display()),
        None => style("none yet — run `waspy sync`").yellow().to_string(),
    };
    let sync_line = match &last {
        Some(entry) if entry["ok"].as_bool() == Some(true) => format!(
            "✓ copied {} ago in {} ms · last checked {} ago",
            ago(now - entry["at"].as_i64().unwrap_or(0)),
            entry["ms"],
            ago(now - entry["checked"].as_i64().unwrap_or(0))
        ),
        Some(entry) => style(format!("✗ {}", entry["error"].as_str().unwrap_or("failed"))).red().to_string(),
        None => style("never").yellow().to_string(),
    };
    let job_line = match job_loaded() {
        true => "loaded — checks every 2 minutes, copies when something changed".to_string(),
        false => style("not installed — run bin/install").yellow().to_string(),
    };
    for (label, value) in [("live database", live_line), ("copy", copy_line), ("last sync", sync_line), ("sync job", job_line)] {
        println!("{}  {value}", style(format!("{label:>13}")).dim());
    }
    Ok(())
}

fn job_loaded() -> bool {
    let uid = std::process::Command::new("id").arg("-u").output().ok();
    let uid = uid.map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string()).unwrap_or_default();
    std::process::Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{JOB}")])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}
