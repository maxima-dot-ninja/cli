use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// An OAuth token. A dedicated one from `heroku authorizations:create -d hku -s identity,read-protected`
    /// never expires and cannot reach anything but reads and config vars.
    #[serde(default)]
    pub api_key: String,
    /// The team `hku apps` lists when none is named. Empty means every app the token can reach.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub team: String,
}

/// Always `~/.config/hku/config.json` (or `$XDG_CONFIG_HOME/hku/`), like every other tool on
/// this machine.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("hku"));
        }
    }
    let home = dirs::home_dir().context("Could not find your home directory")?;
    Ok(home.join(".config").join("hku"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    let mut config: Config = match fs::read_to_string(&path) {
        Ok(text) => {
            serde_json::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?
        }
        Err(_) => Config::default(),
    };

    // Environment wins, so a key exported in the shell, or the one vaulty hands a persona, overrides the file.
    if let Some(key) = vaultykeys::get_for("hku", "HEROKU_API_KEY") {
        if !key.is_empty() {
            config.api_key = key;
        }
    }
    if let Some(team) = vaultykeys::get_for("hku", "HEROKU_TEAM") {
        if !team.is_empty() {
            config.team = team;
        }
    }
    config.api_key = config.api_key.trim().to_string();
    config.team = config.team.trim().to_string();
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(config)? + "\n")
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
        "Heroku is not configured.\n\n\
         Make a token that can read apps and config vars, and nothing else:\n  \
         heroku authorizations:create -d hku -s identity,read-protected\n\n\
         Set it with `vaulty secrets set hku <NAME> <value>`:\n  \
         HEROKU_API_KEY=\"...\"   the token's value\n  \
         HEROKU_TEAM=\"...\"      optional, the team `hku apps` lists by default\n\n\
         Or run `hku setup`, which checks them against the API and writes {path}"
    )
}
