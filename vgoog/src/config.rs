use crate::error::{Result, VgoogError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Name of the currently active account
    #[serde(default = "default_active")]
    pub active_account: String,

    /// Named accounts keyed by profile name
    #[serde(default)]
    pub accounts: BTreeMap<String, Account>,

    /// Legacy single-account field — migrated on load
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

fn default_active() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub label: String,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default = "default_expiry")]
    pub token_expiry: DateTime<Utc>,
}

fn default_expiry() -> DateTime<Utc> {
    Utc::now()
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        if let Ok(dir) = std::env::var("VGOOG_CONFIG_DIR") {
            return Ok(PathBuf::from(dir));
        }
        let dir = dirs::config_dir()
            .ok_or_else(|| VgoogError::Config("Cannot find config directory".into()))?
            .join("vgoog");
        Ok(dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Err(VgoogError::Config(
                "Config not found. Run `vgoog` to set up.".into(),
            ));
        }
        let content = std::fs::read_to_string(&path)?;
        let mut config: Config =
            toml::from_str(&content).map_err(|e| VgoogError::Config(e.to_string()))?;

        // Migrate legacy single-account config
        if let Some(legacy_auth) = config.auth.take() {
            if !config.accounts.contains_key("default") {
                config.accounts.insert(
                    "default".to_string(),
                    Account {
                        label: "Default Account".to_string(),
                        auth: legacy_auth,
                    },
                );
                config.active_account = "default".to_string();
                // Save migrated config
                let _ = config.save();
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = Self::config_path()?;
        let content =
            toml::to_string_pretty(self).map_err(|e| VgoogError::Config(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn exists() -> bool {
        Self::config_path().ok().is_some_and(|p| p.exists())
    }

    /// Get the currently active account
    pub fn active(&self) -> Result<&Account> {
        self.accounts
            .get(&self.active_account)
            .ok_or_else(|| VgoogError::Config(format!("Account '{}' not found", self.active_account)))
    }

    /// Get the currently active account mutably
    pub fn active_mut(&mut self) -> Result<&mut Account> {
        let name = self.active_account.clone();
        self.accounts
            .get_mut(&name)
            .ok_or_else(|| VgoogError::Config(format!("Account '{name}' not found")))
    }

    /// List all account names
    pub fn account_names(&self) -> Vec<&String> {
        self.accounts.keys().collect()
    }

    /// Add a new account
    pub fn add_account(&mut self, name: String, account: Account) {
        self.accounts.insert(name, account);
    }

    /// Remove an account
    pub fn remove_account(&mut self, name: &str) -> bool {
        if self.accounts.remove(name).is_some() {
            if self.active_account == name {
                self.active_account = self
                    .accounts
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_default();
            }
            true
        } else {
            false
        }
    }

    /// Switch active account
    pub fn switch_account(&mut self, name: &str) -> bool {
        if self.accounts.contains_key(name) {
            self.active_account = name.to_string();
            true
        } else {
            false
        }
    }

    /// Check if active account has valid credentials
    pub fn active_is_valid(&self) -> bool {
        self.active().is_ok_and(|a| {
            !a.auth.client_id.is_empty()
                && !a.auth.client_secret.is_empty()
                && !a.auth.access_token.is_empty()
                && !a.auth.refresh_token.is_empty()
        })
    }

    /// Build a legacy-compatible Config for a single account (used by GoogleClient)
    pub fn for_active_account(&self) -> Result<SingleAccountConfig> {
        let account = self.active()?;
        Ok(SingleAccountConfig {
            auth: account.auth.clone(),
            config_ref: self.clone(),
        })
    }
}

/// A view of a single account's config, used by GoogleClient
#[derive(Debug, Clone)]
pub struct SingleAccountConfig {
    pub auth: AuthConfig,
    pub config_ref: Config,
}

impl SingleAccountConfig {
    /// Save updated tokens back to the multi-account config
    pub fn save(&self) -> Result<()> {
        let mut config = self.config_ref.clone();
        if let Some(account) = config.accounts.get_mut(&config.active_account) {
            account.auth = self.auth.clone();
        }
        config.save()
    }
}
