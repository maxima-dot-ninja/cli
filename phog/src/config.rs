use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// The app host, where the private API lives. US cloud by default; EU is
/// `https://eu.posthog.com`, self-hosted is wherever you put it.
pub const DEFAULT_HOST: &str = "https://us.posthog.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// A personal API key (phx_…), never the project key — that one can only write events.
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default = "default_host")]
    pub host: String,
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}

/// Always `~/.config/phog/config.toml` (or `$XDG_CONFIG_HOME/phog/`), like every
/// other tool on this machine.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("phog"));
        }
    }
    let home = dirs::home_dir().context("Could not find your home directory")?;
    Ok(home.join(".config").join("phog"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    let mut config: Config = match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?,
        Err(_) => Config { host: default_host(), ..Default::default() },
    };

    // Environment wins, so a key exported in the shell overrides the file.
    if let Ok(key) = std::env::var("POSTHOG_PERSONAL_API_KEY") {
        if !key.is_empty() {
            config.api_key = key;
        }
    }
    if let Ok(project) = std::env::var("POSTHOG_PROJECT_ID") {
        if !project.is_empty() {
            config.project_id = project;
        }
    }
    if let Ok(host) = std::env::var("POSTHOG_APP_HOST") {
        if !host.is_empty() {
            config.host = host;
        }
    }
    if config.host.is_empty() {
        config.host = default_host();
    }
    config.host = config.host.trim_end_matches('/').to_string();
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

pub fn missing_help() -> String {
    let path = config_path().map(|p| p.display().to_string()).unwrap_or_default();
    format!(
        "PostHog is not configured.\n\n\
         Set these (in ~/.config/secrets.env, recommended):\n  \
         export POSTHOG_PERSONAL_API_KEY=\"phx_...\"   PostHog → Settings → Personal API keys\n  \
         export POSTHOG_PROJECT_ID=\"12345\"           the number in the project's URL\n  \
         export POSTHOG_APP_HOST=\"{DEFAULT_HOST}\"    or eu.posthog.com\n\n\
         Or run `phog setup`, which checks them against the API and writes {path}"
    )
}
