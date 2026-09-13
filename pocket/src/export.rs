use crate::api::{self, Client};
use crate::format;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

/// Each recording takes about a quarter second to fetch, so four at a time is
/// most of the speedup while staying gentle on the API.
const WORKERS: usize = 4;

pub fn one(client: &Client, id: &str) {
    print!("{}", export(client, id).1);
}

/// Every recording, each printed as one block when it lands. Failures are
/// named at the end; rerunning retries them along with everything else.
pub fn all(client: &Client, ids: &[&str]) {
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| loop {
                let Some(id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) else { return };
                let (ok, log) = export(client, id);
                print!("{log}");
                if !ok {
                    failed.lock().unwrap().push(*id);
                }
            });
        }
    });
    let failed = failed.into_inner().unwrap();
    println!("✓ Exported {} recordings to {}", ids.len() - failed.len(), crate::export_root().display());
    if !failed.is_empty() {
        println!("✗ {} failed: {}", failed.len(), failed.join(", "));
    }
}

/// One recording to disk. The log comes back whole rather than printed line by
/// line, so exports running side by side never interleave.
fn export(client: &Client, id: &str) -> (bool, String) {
    let endpoint = api::recording_endpoint(id);
    let log = format!("→ GET {endpoint}\n");
    match write(client, &endpoint) {
        Ok(lines) => (true, log + &lines),
        Err(line) => (false, log + &line + "\n"),
    }
}

fn write(client: &Client, endpoint: &str) -> Result<String, String> {
    let rec = client.get(endpoint)?;
    if rec.is_null() {
        return Err("✗ the answer held no recording".to_string());
    }
    let dir = crate::export_root().join(format::folder_name(&rec));
    fs::create_dir_all(&dir).map_err(|err| format!("✗ {}: {err}", dir.display()))?;

    let text = format::transcript_text(&rec["transcript"]);
    save(&dir.join("transcript.txt"), &text)?;
    let markdown = format::summary_markdown(&rec);
    save(&dir.join("summary.md"), &markdown)?;
    if text.is_empty() {
        save(&dir.join("raw.json"), &serde_json::to_string_pretty(&rec).unwrap_or_default())?;
    }

    Ok(format!(
        "  transcript.txt ({} chars)\n  summary.md ({} chars)\n✓ {}\n\n",
        text.encode_utf16().count(),
        markdown.encode_utf16().count(),
        dir.display()
    ))
}

/// Written beside the target and renamed over it, so an interrupted export
/// never leaves half a file for search to index. The temp name is fixed, so a
/// leftover one is simply overwritten by the next run.
fn save(path: &Path, content: &str) -> Result<(), String> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    fs::write(&tmp, content)
        .and_then(|_| fs::rename(&tmp, path))
        .map_err(|err| format!("✗ {}: {err}", path.display()))
}
