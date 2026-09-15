//! Regex or literal search over every readable text file under the root.

use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::path::Path;

/// Files bigger than this are skipped rather than read into memory.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// Longer lines are cut in the snippet; the line number is still exact.
const SNIPPET_CHARS: usize = 240;

pub fn pattern(text: &str, literal: bool, ignore_case: bool) -> Result<Regex> {
    let source = if literal { regex::escape(text) } else { text.to_string() };
    RegexBuilder::new(&source).case_insensitive(ignore_case).build().with_context(|| format!("bad pattern: {text}"))
}

fn snippet(line: &str) -> String {
    let line = line.trim_end();
    match line.char_indices().nth(SNIPPET_CHARS) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}

/// Hits in one file, or none when it is binary, too big, or unreadable.
fn hits_in(path: &Path, re: &Regex, room: usize) -> Vec<Value> {
    let too_big = std::fs::metadata(path).map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true);
    if too_big || room == 0 {
        return vec![];
    }
    let Ok(bytes) = std::fs::read(path) else { return vec![] };
    if crate::read::is_binary(&bytes) {
        return vec![];
    }
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .enumerate()
        .filter(|(_, line)| re.is_match(line))
        .take(room)
        .map(|(i, line)| json!({ "path": path.display().to_string(), "line": i + 1, "snippet": snippet(line) }))
        .collect()
}

/// At most `max` hits, files in the order given, so a capped result is still deterministic.
pub fn search(files: &[std::path::PathBuf], re: &Regex, max: usize) -> Vec<Value> {
    let mut hits = Vec::new();
    for path in files {
        hits.extend(hits_in(path, re, max - hits.len()));
        if hits.len() >= max {
            break;
        }
    }
    hits
}
