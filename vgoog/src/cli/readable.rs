// A Gmail message as something a reader can read.
//
// Gmail's `format=full` is the MIME tree with every body base64url-encoded: a two-line email came
// back as ~44 KB of encoded HTML. An agent reading it saw noise, spent a paid call per message
// asking another model to decode it, and a triage of three emails ran 25 minutes before it was
// stopped. This keeps what a person reads — who, when, the subject, the text — and names the
// attachments without their bytes.
//
// Used unless `format` names one of Gmail's other shapes — "raw" for the RFC 2822 source,
// "metadata" or "minimal".

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};

const HEADERS: [&str; 6] = ["From", "To", "Cc", "Date", "Subject", "Reply-To"];

fn decode(data: &str) -> String {
    URL_SAFE_NO_PAD
        .decode(data.trim_end_matches('='))
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default()
}

/// Every leaf part of the MIME tree, depth first.
fn leaves<'a>(part: &'a Value, into: &mut Vec<&'a Value>) {
    match part.get("parts").and_then(Value::as_array) {
        Some(children) => children.iter().for_each(|child| leaves(child, into)),
        None => into.push(part),
    }
}

fn mime(part: &Value) -> &str {
    part.get("mimeType").and_then(Value::as_str).unwrap_or("")
}

fn body_of(part: &Value) -> String {
    decode(part.pointer("/body/data").and_then(Value::as_str).unwrap_or(""))
}

/// HTML to text, crudely: drop style and script blocks, turn breaks into newlines, drop tags,
/// decode the entities mail actually uses, collapse the blank lines.
fn strip_html(html: &str) -> String {
    let mut text = html.to_string();
    for block in ["style", "script", "head"] {
        // ASCII lowering keeps every byte offset where it was, so the ranges below stay on char
        // boundaries in the original text.
        while let Some(start) = text.to_ascii_lowercase().find(&format!("<{block}")) {
            let close = format!("</{block}>");
            let Some(end) = text.to_ascii_lowercase()[start..].find(&close) else { break };
            text.replace_range(start..start + end + close.len(), "");
        }
    }
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for c in text.chars() {
        match (c, in_tag) {
            ('<', _) => {
                in_tag = true;
                tag.clear();
            }
            ('>', true) => {
                in_tag = false;
                let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_lowercase();
                if ["br", "p", "div", "tr", "li", "h1", "h2", "h3", "table"].contains(&name.as_str()) {
                    out.push('\n');
                }
            }
            (_, true) => tag.push(c),
            (_, false) => out.push(c),
        }
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace('\u{34f}', "")
        .replace('\u{200c}', "");
    decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn message(full: &Value) -> Value {
    let payload = full.get("payload").cloned().unwrap_or(Value::Null);
    let headers: serde_json::Map<String, Value> = payload
        .get("headers")
        .and_then(Value::as_array)
        .map(|all| {
            all.iter()
                .filter_map(|h| Some((h.get("name")?.as_str()?, h.get("value")?.clone())))
                .filter(|(name, _)| HEADERS.iter().any(|wanted| wanted.eq_ignore_ascii_case(name)))
                .map(|(name, value)| (name.to_lowercase(), value))
                .collect()
        })
        .unwrap_or_default();

    let mut parts = Vec::new();
    leaves(&payload, &mut parts);

    let plain = parts.iter().find(|p| mime(p) == "text/plain").map(|p| body_of(p)).filter(|t| !t.trim().is_empty());
    let html = parts.iter().find(|p| mime(p) == "text/html").map(|p| strip_html(&body_of(p)));
    let text = plain.or(html).unwrap_or_default();

    let attachments: Vec<Value> = parts
        .iter()
        .filter(|p| !p.get("filename").and_then(Value::as_str).unwrap_or("").is_empty())
        .map(|p| {
            json!({
                "filename": p.get("filename"),
                "mimeType": mime(p),
                "size": p.pointer("/body/size"),
                "attachmentId": p.pointer("/body/attachmentId"),
            })
        })
        .collect();

    json!({
        "id": full.get("id"),
        "threadId": full.get("threadId"),
        "labelIds": full.get("labelIds"),
        "headers": headers,
        "snippet": full.get("snippet"),
        "text": text.trim(),
        "attachments": attachments,
    })
}

/// A thread, each message made readable. Anything that is not the shape expected passes through.
pub fn thread(full: &Value) -> Value {
    let Some(messages) = full.get("messages").and_then(Value::as_array) else { return full.clone() };
    json!({
        "id": full.get("id"),
        "messages": messages.iter().map(message).collect::<Vec<_>>(),
    })
}
