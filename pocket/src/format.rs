use crate::search::Recording;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

/// A string field, or "" when it is missing or not a string.
pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The first of these fields holding a non-empty string, the way `a || b` reads.
fn first<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().map(|key| text(value, key)).find(|s| !s.is_empty()).unwrap_or("")
}

fn or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

/// NFKD first, so an accented letter keeps its base letter instead of turning
/// into a hyphen. Existing export folders were named this way, so it has to
/// stay byte-for-byte the same or a re-export would land in a new folder.
fn slugify(text: &str) -> String {
    let decomposed: String = text.to_lowercase().nfkd().collect();
    let parts: Vec<&str> = decomposed
        .split(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect();
    parts.join("-")
}

/// `YYYY-MM-DD`, digits and dashes in place.
pub fn is_day(text: &str) -> bool {
    text.len() == 10
        && text
            .bytes()
            .enumerate()
            .all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() })
}

/// The UTC day of the recording time, else the creation time. The API sends
/// both as `2026-09-11T14:35:21Z`, so the day is the first ten characters.
pub fn recording_date(rec: &Value) -> String {
    let day = first(rec, &["recording_at", "created_at"]).get(..10).unwrap_or("");
    if !is_day(day) {
        return "unknown-date".to_string();
    }
    day.to_string()
}

pub fn recording_label(rec: &Value) -> String {
    format!("{}  {}", recording_date(rec), or(text(rec, "title"), "(untitled)"))
}

pub fn folder_name(rec: &Value) -> String {
    let slug: String = slugify(or(text(rec, "title"), "untitled")).chars().take(60).collect();
    format!("{}-{}", or(&slug, "untitled"), recording_date(rec))
}

fn transcript_line(seg: &Value) -> String {
    let speaker = first(seg, &["speaker_name", "speaker"]);
    let text = first(seg, &["text", "content"]);
    if speaker.is_empty() {
        return text.to_string();
    }
    format!("{speaker}: {text}")
}

/// The transcript arrives as { metadata, segments, text }.
pub fn transcript_text(transcript: &Value) -> String {
    match transcript.get("segments").unwrap_or(transcript) {
        Value::String(text) => text.clone(),
        Value::Array(segments) => segments.iter().map(transcript_line).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn action_lines(action: &Value) -> Vec<String> {
    let done = if action["isCompleted"].as_bool() == Some(true) { "x" } else { " " };
    let due = text(action, "dueDate");
    let due = if due.is_empty() { String::new() } else { format!(" (due {due})") };
    let mut lines = vec![format!("- [{done}] {}{due}", text(action, "label"))];
    let context = text(action, "context");
    if !context.is_empty() {
        lines.push(format!("  - {context}"));
    }
    lines
}

/// Each summarization carries its content under v2 (summary.markdown, actionItems).
fn summary_block(summ: &Value) -> Vec<String> {
    let content = summ.pointer("/v2/summary/markdown").and_then(Value::as_str).unwrap_or("");
    let items: Vec<String> = summ
        .pointer("/v2/actionItems/actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(action_lines)
        .collect();
    let mut block = vec![content.to_string(), String::new()];
    if !items.is_empty() {
        block.extend(["## Action items".to_string(), String::new()]);
        block.extend(items);
        block.push(String::new());
    }
    block
}

pub fn summary_markdown(rec: &Value) -> String {
    let duration = match &rec["duration"] {
        Value::Number(n) if n.as_f64() != Some(0.0) => n.to_string(),
        _ => "?".to_string(),
    };
    let mut lines = vec![
        format!("# {}", or(text(rec, "title"), "Untitled recording")),
        String::new(),
        format!("- **Date:** {}", recording_date(rec)),
        format!("- **Duration:** {duration}s"),
        format!("- **Recording ID:** {}", text(rec, "id")),
        String::new(),
    ];
    let blocks: Vec<String> = rec["summarizations"]
        .as_object()
        .into_iter()
        .flat_map(|summaries| summaries.values())
        .flat_map(summary_block)
        .collect();
    if blocks.is_empty() {
        lines.push("_No summary available._".to_string());
    }
    lines.extend(blocks);
    lines.join("\n")
}

fn hit_line(hit: &Value) -> String {
    let snippet = text(hit, "snippet");
    let quote = match snippet.char_indices().nth(220) {
        Some((cut, _)) => format!("{}…", &snippet[..cut]),
        None => snippet.to_string(),
    };
    format!("      {}:{}  {quote}", text(hit, "file"), hit["line"])
}

/// Best two chunks per recording — enough to judge relevance without a wall of text.
fn result_block(result: &Recording, rank: usize) -> Vec<String> {
    let percent = format!("{}%", (result.score.as_f64().unwrap_or(0.0) * 100.0).round() as i64);
    let mut block = vec![
        format!("{rank:>2}. {percent:>4}  {}  ·  {}", result.title, or(&result.date, "unknown date")),
        format!("      {}", result.dir),
    ];
    block.extend(result.hits.iter().take(2).map(hit_line));
    block.push(String::new());
    block
}

pub fn search_results_text(results: &[Recording]) -> String {
    if results.is_empty() {
        return "\nNo matching conversations.\n".to_string();
    }
    let mut lines = vec![String::new()];
    for (i, result) in results.iter().enumerate() {
        lines.extend(result_block(result, i + 1));
    }
    lines.push(format!("{} matching recordings", results.len()));
    lines.join("\n")
}
