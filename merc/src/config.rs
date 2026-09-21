//! Where the API token lives and which Mercury it talks to.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Where the token came from. Worth knowing when it is rejected: a stale value
/// exported in one terminal outlives any edit to the file.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Source {
    #[default]
    Nowhere,
    Environment,
    File,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Mercury issues these with a `secret-token:` prefix; keep it.
    #[serde(default)]
    pub api_key: String,
    /// Optional second token used for reads.
    ///
    /// Mercury forces an IP allow-list onto any token that can write, so the main
    /// token stops working the moment you change network. A `Read Only` token has
    /// no allow-list and works from anywhere, which covers every GET this tool
    /// makes. Money still moves on `api_key`.
    #[serde(default)]
    pub read_key: String,
    /// Talk to the sandbox unless told otherwise
    #[serde(default)]
    pub sandbox: bool,
    /// Only for building an OAuth2 integration; not needed for an API token
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(skip)]
    pub key_source: Source,
}

impl Config {
    /// Sandbox and production are separate banks with separate tokens, so the
    /// host is decided once, here, and shown on every confirmation prompt.
    pub fn base_url(&self) -> &'static str {
        match self.sandbox {
            true => "https://api-sandbox.mercury.com/api/v1",
            false => "https://api.mercury.com/api/v1",
        }
    }

    pub fn oauth_url(&self) -> &'static str {
        match self.sandbox {
            true => "https://oauth2-sandbox.mercury.com",
            false => "https://oauth2.mercury.com",
        }
    }

    /// The token a given operation should travel on: reads prefer the allow-list-free
    /// read token, writes must use the real one.
    pub fn key_for(&self, mutates: bool) -> &str {
        match mutates || self.read_key.is_empty() {
            true => &self.api_key,
            false => &self.read_key,
        }
    }

    pub fn environment(&self) -> &'static str {
        match self.sandbox {
            true => "sandbox",
            false => "production",
        }
    }
}

/// `~/.config/merc/config.toml`, matching every other tool in this repo rather
/// than macOS's `~/Library/Application Support`, where nobody looks.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("merc"));
        }
    }
    Ok(dirs::home_dir().context("Could not find your home directory")?.join(".config").join("merc"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    let mut config: Config = match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?,
        Err(_) => Config::default(),
    };

    config.api_key = config.api_key.trim().to_string();
    config.read_key = config.read_key.trim().to_string();
    if !config.api_key.is_empty() {
        config.key_source = Source::File;
    }

    // The vault wins over this tool's own config file, and an exported variable wins over both.
    // Resolution lives in `vaultykeys` — nothing sources secrets into the shell any more, so a
    // key has to be READ rather than inherited.
    for (variable, field) in [
        ("MERCURY_API_KEY", &mut config.api_key),
        ("MERCURY_READ_KEY", &mut config.read_key),
        ("MERCURY_CLIENT_ID", &mut config.client_id),
        ("MERCURY_CLIENT_SECRET", &mut config.client_secret),
    ] {
        if let Some(value) = vaultykeys::get_for("merc", variable) {
            if !value.is_empty() {
                *field = value;
            }
        }
    }
    config.read_key = config.read_key.trim().to_string();
    if vaultykeys::get_for("merc", "MERCURY_API_KEY").is_some_and(|value| !value.trim().is_empty()) {
        config.api_key = config.api_key.trim().to_string();
        config.key_source = Source::Environment;
    }
    if vaultykeys::get("MERCURY_SANDBOX").is_some_and(|v| v == "1" || v == "true") {
        config.sandbox = true;
    }
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, toml::to_string_pretty(config)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    restrict(&path);
    Ok(())
}

/// The file holds a token that can move money. Owner-only.
#[cfg(unix)]
fn restrict(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &PathBuf) {}

/// What to check when Mercury says no. Every line here is something that has
/// actually gone wrong rather than a list of possibilities.
pub fn token_advice(config: &Config) -> String {
    let where_from = match config.key_source {
        Source::Environment => "the MERCURY_API_KEY environment variable".to_string(),
        Source::File => config_path().map(|p| p.display().to_string()).unwrap_or_default(),
        Source::Nowhere => return missing_key_help(),
    };

    let mut lines = vec![format!("The token came from {where_from} ({} characters).", config.api_key.len())];
    if !config.api_key.starts_with("secret-token:") {
        lines.push("It does not start with `secret-token:` — Mercury's tokens include that prefix.".into());
    }
    if config.api_key.contains("mercury_sandbox") && !config.sandbox {
        lines.push("It is a sandbox token, but this is production. Add --sandbox.".into());
    }
    if config.api_key.contains("mercury_production") && config.sandbox {
        lines.push("It is a production token, but this is the sandbox. Drop --sandbox.".into());
    }
    if config.key_source == Source::Environment {
        lines.push(
            "If you edited the file it came from, this shell still holds the old value — \
             run `vaulty secrets set merc MERCURY_READ_KEY <token>`."
                .into(),
        );
    }
    lines.join("\n")
}

/// What to do when Mercury blocks the IP rather than the token.
///
/// Mercury reports this as a 401, which reads as "bad token" and sends you rotating a
/// key that was never wrong. It is worth spelling out both exits, because one of them
/// removes the problem permanently instead of postponing it to the next network.
pub fn ip_advice(ip: &str, mutates: bool, has_read_key: bool) -> String {
    let seen = match ip.is_empty() {
        true => "This machine's IP".to_string(),
        false => format!("This machine's IP ({ip})"),
    };
    let mut lines =
        vec![format!("{seen} is not on this token's allow-list. The token itself is fine."), String::new()];

    lines.push("Add it: mercury.com → Settings → API Tokens → this token → IP allow-list.".into());

    if mutates {
        lines.push(String::new());
        lines.push(
            "There is no way around it for this command — Mercury requires an allow-list on \
             any token that can write."
                .into(),
        );
        return lines.join("\n");
    }

    if has_read_key {
        lines.push(String::new());
        lines.push(
            "MERCURY_READ_KEY is set but was not used here, so it is a write-scoped token. \
             A true `Read Only` token is the one with no allow-list."
                .into(),
        );
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push("Or stop needing one. This was a read, and a `Read Only` token has no allow-list".into());
    lines.push("at all — it works from any network. Create one alongside the token you have:".into());
    lines.push("  vaulty secrets set merc MERCURY_READ_KEY secret-token:...".into());
    lines.push("merc then reads on that token and keeps the current one for moving money.".into());
    lines.join("\n")
}

pub fn missing_key_help() -> String {
    let path = config_path().map(|p| p.display().to_string()).unwrap_or_default();
    format!(
        "No API token found.\n\n\
         Create one at https://mercury.com → Settings → API Tokens, then:\n  \
         vaulty secrets set merc MERCURY_API_KEY secret-token:...\n\n\
         Or put it in {path}:\n  \
         api_key = \"secret-token:...\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(key: &str, source: Source, sandbox: bool) -> Config {
        Config { api_key: key.into(), key_source: source, sandbox, ..Config::default() }
    }

    #[test]
    fn a_rejected_token_says_where_it_came_from() {
        let advice = token_advice(&with("secret-token:mercury_production_abc", Source::Environment, false));
        assert!(advice.contains("MERCURY_API_KEY environment variable"), "{advice}");
        assert!(advice.contains("35 characters"), "{advice}");
        // The one that actually catches people: the file was fixed, the shell was not.
        assert!(advice.contains("open a new terminal"), "{advice}");
    }

    #[test]
    fn a_token_from_the_file_does_not_blame_the_shell() {
        let advice = token_advice(&with("secret-token:mercury_production_abc", Source::File, false));
        assert!(advice.contains("config.toml"), "{advice}");
        assert!(!advice.contains("open a new terminal"), "{advice}");
    }

    #[test]
    fn the_wrong_environment_is_named_outright() {
        let advice = token_advice(&with("secret-token:mercury_sandbox_abc", Source::Environment, false));
        assert!(advice.contains("Add --sandbox"), "{advice}");

        let advice = token_advice(&with("secret-token:mercury_production_abc", Source::Environment, true));
        assert!(advice.contains("Drop --sandbox"), "{advice}");
    }

    #[test]
    fn a_missing_prefix_is_pointed_out() {
        let advice = token_advice(&with("mercury_production_abc", Source::Environment, false));
        assert!(advice.contains("secret-token:"), "{advice}");
    }

    #[test]
    fn no_token_at_all_gets_the_setup_instructions_instead() {
        let advice = token_advice(&with("", Source::Nowhere, false));
        assert!(advice.contains("Settings → API Tokens"), "{advice}");
    }
}
