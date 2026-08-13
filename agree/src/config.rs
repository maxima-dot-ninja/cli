use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const BASE_URL: &str = "https://secure.agree.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api_key: String,
    /// Default currency for amounts entered without one
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// anthropic | openai | gemini | ollama
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self { provider: String::new(), model: String::new(), api_key: String::new() }
    }
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Always `~/.config/agree/config.toml` (or `$XDG_CONFIG_HOME/agree/`).
///
/// Deliberately not `dirs::config_dir()`, which on macOS points at
/// `~/Library/Application Support` — nobody goes looking there, and every other
/// tool on this machine (git, gh, nvim, stripe, pocket) uses `~/.config`.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("agree"));
        }
    }
    let home = dirs::home_dir().context("Could not find your home directory")?;
    Ok(home.join(".config").join("agree"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    let mut config: Config = match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text)
            .with_context(|| format!("Failed to parse {}", path.display()))?,
        Err(_) => Config { currency: default_currency(), ..Default::default() },
    };

    // Environment wins, so a key exported in the shell overrides the file.
    if let Ok(key) = std::env::var("AGREE_API_KEY") {
        config.api_key = key;
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
    restrict_permissions(&path);
    Ok(())
}

/// The file holds an API key, so keep it owner-only.
#[cfg(unix)]
fn restrict_permissions(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &PathBuf) {}

pub fn missing_key_help() -> String {
    let path = config_path().map(|p| p.display().to_string()).unwrap_or_default();
    format!(
        "No API key found.\n\n\
         Set one of these:\n  \
         export AGREE_API_KEY=\"...\"        (recommended, in ~/.config/secrets.env)\n  \
         agree setup                        (writes {path})"
    )
}
