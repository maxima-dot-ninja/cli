//! Where this machine's secrets live.
//!
//! They used to sit in `~/.config/secrets.env`, which `~/.zshrc` sourced — so every CLI read its
//! keys out of the process ENVIRONMENT and `std::env::var` was enough. That file is gone. Secrets
//! now live in `~/.vaulty/.secrets/`, one file per skill, and nothing sources them into a shell.
//!
//! So a CLI has to READ them. That is the whole reason this crate exists: the resolution order
//! lives here once, rather than being copied into a dozen `config.rs` files that then drift.
//!
//! ```no_run
//! let key = vaultykeys::get("MERCURY_READ_KEY");           // any file
//! let key = vaultykeys::get_for("merc", "MERCURY_READ_KEY"); // prefer merc.env
//! ```
//!
//! File format is plain `KEY=value`, one per line, no `export`, mode 0600. A leading `export ` is
//! tolerated on the way in so a file pasted over from the old world still reads.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Shared keys that belong to no single skill.
pub const SHARED: &str = "vaulty";

/// `~/.vaulty/.secrets`, or wherever `VAULTY_SECRETS_DIR` points. The override exists for tests
/// and for a second install on one machine; nothing else should set it.
pub fn secrets_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("VAULTY_SECRETS_DIR") {
        if !explicit.trim().is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".vaulty").join(".secrets")
}

fn file_for(skill: &str) -> PathBuf {
    secrets_dir().join(format!("{skill}.env"))
}

/// The name a line sets, if it sets one.
fn name_of(line: &str) -> Option<&str> {
    let body = line.trim();
    if body.starts_with('#') || body.is_empty() {
        return None;
    }
    let body = body.strip_prefix("export ").unwrap_or(body).trim_start();
    let (name, _) = body.split_once('=')?;
    let name = name.trim();
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    valid.then_some(name)
}

fn value_of(line: &str) -> String {
    let body = line.trim();
    let body = body.strip_prefix("export ").unwrap_or(body);
    let Some((_, value)) = body.split_once('=') else { return String::new() };
    value.trim().trim_matches('\'').trim_matches('"').to_string()
}

fn read_file(path: &PathBuf) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(body) = std::fs::read_to_string(path) else { return map };
    for line in body.lines() {
        let Some(name) = name_of(line) else { continue };
        let value = value_of(line);
        if !value.is_empty() {
            map.insert(name.to_string(), value);
        }
    }
    map
}

/// One secret, resolved in order:
///
/// 1. the process environment — an explicit override on the command line still wins
/// 2. `~/.vaulty/.secrets/<skill>.env` when a skill is named
/// 3. `~/.vaulty/.secrets/vaulty.env` — the shared keys
/// 4. every other `*.env` in that directory, so a key filed under an unexpected name still works
pub fn get_for(skill: &str, name: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if !skill.is_empty() {
        if let Some(value) = read_file(&file_for(skill)).remove(name) {
            return Some(value);
        }
    }
    if let Some(value) = read_file(&file_for(SHARED)).remove(name) {
        return Some(value);
    }

    let Ok(entries) = std::fs::read_dir(secrets_dir()) else { return None };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|e| e == "env").unwrap_or(false))
        .collect();
    files.sort();
    files.iter().find_map(|path| read_file(path).remove(name))
}

/// The same, without naming a skill.
pub fn get(name: &str) -> Option<String> {
    get_for("", name)
}

/// The same, as a plain String — empty when absent. For call sites that were `env::var(..)
/// .unwrap_or_default()`.
pub fn get_or_empty(name: &str) -> String {
    get(name).unwrap_or_default()
}

pub fn has(name: &str) -> bool {
    get(name).map(|value| !value.trim().is_empty()).unwrap_or(false)
}

/// One line telling a person how to set a key they are missing. Every CLI's setup help should end
/// with this rather than inventing its own wording.
pub fn how_to_set(skill: &str, name: &str) -> String {
    format!(
        "Set it with:\n    vaulty secrets set {} {name} <value>\n\nIt lands in {}",
        if skill.is_empty() { SHARED } else { skill },
        file_for(if skill.is_empty() { SHARED } else { skill }).display()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_a_skill_file_then_the_shared_one() {
        let dir = std::env::temp_dir().join("vaultykeys-test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("merc.env"), "MERCURY_READ_KEY=from-merc\n").unwrap();
        std::fs::write(dir.join("vaulty.env"), "export SHARED_ONE='from-vaulty'\n").unwrap();
        std::env::set_var("VAULTY_SECRETS_DIR", &dir);

        assert_eq!(super::get_for("merc", "MERCURY_READ_KEY").as_deref(), Some("from-merc"));
        assert_eq!(super::get("SHARED_ONE").as_deref(), Some("from-vaulty"));
        assert_eq!(super::get("NOT_THERE"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
