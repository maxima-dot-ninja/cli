use crate::format;
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::process::{Command, Stdio};

const COLLECTION: &str = "pocket";
// Transcripts are .txt, summaries are .md — qmd only globs **/*.md by default.
const MASK: &str = "**/*.{md,txt}";
// The bare `qmd` name on npm is a dead placeholder, so the scoped package it is.
// `--index index` pins the global index: without it qmd walks up from cwd and
// silently uses a project-local .qmd index that knows nothing about pocket.
const QMD: [&str; 4] = ["-y", "@tobilu/qmd", "--index", "index"];
const NO_NPX: &str = "Could not run npx. Search runs qmd through it, so it needs Node on your PATH.";
// Hits name their file as qmd://pocket/<folder>/<file>.
const VIRTUAL: &str = "qmd://pocket/";

/// One recording, ranked by its best chunk. Scores stay the JSON values qmd
/// sent, so `--json` prints them exactly as qmd wrote them.
#[derive(Serialize)]
pub struct Recording {
    pub folder: String,
    pub title: String,
    pub date: String,
    pub dir: String,
    pub score: Value,
    pub hits: Vec<Value>,
}

fn qmd(args: &[&str]) -> Command {
    let root = crate::export_root();
    let _ = std::fs::create_dir_all(&root);
    let mut command = Command::new("npx");
    command.args(QMD).args(args).current_dir(root);
    command
}

fn is_indexed() -> bool {
    let config = dirs::home_dir().unwrap_or_default().join(".config").join("qmd").join("index.yml");
    std::fs::read_to_string(config).is_ok_and(|text| text.lines().any(|line| line == "  pocket:"))
}

/// Adding the collection indexes it; updating re-scans an existing one.
/// Embedding only touches documents whose vectors are missing, so this is
/// cheap when warm.
pub fn reindex() -> Result<()> {
    let root = crate::export_root().to_string_lossy().to_string();
    let setup = ["collection", "add", &root, "--name", COLLECTION, "--mask", MASK];
    let scan: &[&str] = if is_indexed() { &["update"] } else { &setup };
    qmd(scan).status().context(NO_NPX)?;
    qmd(&["embed", "-c", COLLECTION]).status().context(NO_NPX)?;
    Ok(())
}

fn ensure_indexed() -> Result<()> {
    if is_indexed() {
        return Ok(());
    }
    println!("First search — building the index (downloads local models once).\n");
    reindex()
}

pub fn search(query: &str, limit: usize, rerank: bool) -> Result<Vec<Recording>> {
    ensure_indexed()?;

    // Ask for more chunks than recordings wanted — several usually share a folder.
    let count = (limit * 4).to_string();
    let mut args = vec!["query", query, "-c", COLLECTION, "-n", &count, "--format", "json"];
    if !rerank {
        args.push("--no-rerank");
    }

    let output = qmd(&args).stderr(Stdio::inherit()).output().context(NO_NPX)?;
    if !output.status.success() || output.stdout.trim_ascii().is_empty() {
        return Ok(Vec::new());
    }
    let hits: Vec<Value> = serde_json::from_slice(&output.stdout).context("qmd did not answer JSON")?;
    Ok(group(hits).into_iter().take(limit).collect())
}

/// Export folders are named <slug>-<YYYY-MM-DD>.
fn describe(folder: &str) -> (String, String) {
    let cut = folder.len().checked_sub(11).filter(|&i| folder.is_char_boundary(i));
    match cut.map(|i| folder.split_at(i)) {
        Some((slug, rest)) if rest.starts_with('-') && format::is_day(&rest[1..]) => {
            (slug.replace('-', " "), rest[1..].to_string())
        }
        _ => (folder.to_string(), String::new()),
    }
}

/// Every hit carries a diff-style "@@ -462,4 @@ (461 before…)" header line.
fn trim(snippet: &str) -> String {
    let body = match snippet.strip_prefix("@@").and_then(|rest| rest.split_once('\n')) {
        Some((_, body)) => body,
        None => snippet,
    };
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn score_of(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

/// One recording produces many chunk hits; fold them into a single result
/// ranked by its best chunk, since the question is "which conversation?".
fn group(hits: Vec<Value>) -> Vec<Recording> {
    let mut recordings: Vec<Recording> = Vec::new();
    for mut hit in hits {
        let path = format::text(&hit, "file").strip_prefix(VIRTUAL).unwrap_or("");
        let Some((folder, file)) = path.split_once('/').filter(|(f, rest)| !f.is_empty() && !rest.is_empty())
        else {
            continue;
        };
        let (folder, file) = (folder.to_string(), file.to_string());
        hit["snippet"] = trim(format::text(&hit, "snippet")).into();
        hit["file"] = file.into();

        let score = hit["score"].clone();
        if let Some(found) = recordings.iter_mut().find(|r| r.folder == folder) {
            if score_of(&score) > score_of(&found.score) {
                found.score = score;
            }
            found.hits.push(hit);
            continue;
        }
        let (title, date) = describe(&folder);
        let dir = crate::export_root().join(&folder).display().to_string();
        recordings.push(Recording { folder, title, date, dir, score, hits: vec![hit] });
    }
    recordings.sort_by(|a, b| score_of(&b.score).total_cmp(&score_of(&a.score)));
    recordings
}
