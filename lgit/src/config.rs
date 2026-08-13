use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitConfig {
    #[serde(default = "default_true")]
    pub auto_push: bool,
    #[serde(default = "default_true")]
    pub pr_link: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    #[serde(default = "default_true")]
    pub color: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            auto_push: true,
            pr_link: true,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { color: true }
    }
}

/// Where config lives: `~/.config/lgit/config.toml`, or `$XDG_CONFIG_HOME/lgit/`.
///
/// Deliberately not `dirs::config_dir()` — on macOS that resolves to
/// `~/Library/Application Support`, where nobody thinks to look. Every other tool
/// on a typical machine (git, gh, nvim) uses `~/.config`, so lgit does too.
pub fn config_path() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("lgit").join("config.toml"));
        }
    }
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".config").join("lgit").join("config.toml"))
}

/// The pre-move location. Still read when the new path has nothing, so an
/// existing install keeps working until the old file is deleted by hand.
fn legacy_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("lgit").join("config.toml"))
}

/// The path config was actually found at, preferring the new location
fn existing_config_path() -> Option<PathBuf> {
    let current = config_path().ok().filter(|p| p.exists());
    current.or_else(|| legacy_config_path().filter(|p| p.exists()))
}

/// Check if config file exists
pub fn config_exists() -> bool {
    existing_config_path().is_some()
}

/// Load configuration from file
pub fn load_config() -> Result<Config> {
    let path = existing_config_path()
        .unwrap_or(config_path()?);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;
    let config: Config = toml::from_str(&content).context("Failed to parse config file")?;
    Ok(config)
}

/// Save configuration to file
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config).context("Failed to serialize config")?;
    fs::write(&path, content).with_context(|| format!("Failed to write config to {}", path.display()))?;
    restrict_permissions(&path);

    Ok(())
}

/// This file holds an API key, so it must not be world-readable.
#[cfg(unix)]
fn restrict_permissions(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &PathBuf) {}

/// Display current configuration
pub fn show_config() -> Result<()> {
    use console::style;

    // Report where config was actually read from, which during the move to
    // ~/.config may still be the old location.
    let Some(path) = existing_config_path() else {
        println!(
            "{} No configuration found. Run {} to set up.",
            style("!").yellow(),
            style("lgit --setup").cyan()
        );
        return Ok(());
    };

    let config = load_config()?;

    println!();
    println!("{}", style("lgit configuration").bold().cyan());
    println!("{}", style("─".repeat(40)).dim());
    println!();
    println!("{}  {}", style("Config file:").dim(), path.display());
    println!();
    println!("{}", style("[provider]").bold());
    println!("  name     = {}", style(&config.provider.name).green());
    println!("  model    = {}", style(&config.provider.model).green());
    println!(
        "  api_key  = {}",
        if config.provider.api_key.is_empty() {
            style("(not set)").dim().to_string()
        } else {
            style("••••••••").dim().to_string()
        }
    );
    println!();
    println!("{}", style("[git]").bold());
    println!("  auto_push = {}", style(config.git.auto_push).green());
    println!("  pr_link   = {}", style(config.git.pr_link).green());
    println!();
    println!("{}", style("[ui]").bold());
    println!("  color = {}", style(config.ui.color).green());
    println!();

    Ok(())
}
