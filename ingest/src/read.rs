//! One file, as JSON: text when it is text, base64 when it is not.

use anyhow::{Context, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Bytes served per file unless --max-bytes or INGEST_MAX_BYTES says otherwise.
pub const DEFAULT_MAX_BYTES: usize = 256 * 1024;

pub fn max_bytes() -> usize {
    std::env::var("INGEST_MAX_BYTES").ok().and_then(|v| v.parse().ok()).filter(|&n| n > 0).unwrap_or(DEFAULT_MAX_BYTES)
}

/// A NUL in the first 8KB is the usual heuristic; text files never carry one.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

pub struct Slice {
    pub size: u64,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

impl Slice {
    pub fn truncated(&self) -> bool {
        self.offset + self.bytes.len() as u64 != self.size.max(self.offset)
    }
}

pub fn slice(path: &Path, offset: u64, limit: usize) -> Result<Slice> {
    let mut file = std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::with_capacity(limit.min(size as usize));
    file.take(limit as u64).read_to_end(&mut bytes)?;
    Ok(Slice { size, offset, bytes })
}

/// Text stays text, with any invalid UTF-8 replaced; binary becomes base64.
pub fn encode(bytes: &[u8], binary: bool) -> String {
    if binary {
        return base64::engine::general_purpose::STANDARD.encode(bytes);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// The `read` reply: everything an agent needs to know whether it saw the whole file.
pub fn json(path: &Path, slice: &Slice, force_base64: bool) -> Value {
    let binary = force_base64 || is_binary(&slice.bytes);
    json!({
        "path": path.display().to_string(),
        "size": slice.size,
        "offset": slice.offset,
        "bytes_read": slice.bytes.len(),
        "truncated": slice.truncated(),
        "binary": binary,
        "encoding": if binary { "base64" } else { "utf-8" },
        "content": encode(&slice.bytes, binary),
    })
}
