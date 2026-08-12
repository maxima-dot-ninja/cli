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

/// Get the config file path (~/.config/lgit/config.toml)
pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("lgit");
    Ok(config_dir.join("config.toml"))
}

/// Check if config file exists
pub fn config_exists() -> bool {
    config_path().map(|p| p.exists()).unwrap_or(false)
}

/// Load configuration from file
pub fn load_config() -> Result<Config> {
    let path = config_path()?;
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

    Ok(())
}

/// Display current configuration
pub fn show_config() -> Result<()> {
    use console::style;

    let path = config_path()?;

    if !path.exists() {
        println!(
            "{} No configuration found. Run {} to set up.",
            style("!").yellow(),
            style("lgit --setup").cyan()
        );
        return Ok(());
    }

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
