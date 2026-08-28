use crate::auth::service_account::ServiceAccountKey;
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

    /// Which credential kind to build from the vault when both are available.
    #[serde(default)]
    pub strategy: Strategy,
}

/// Which way in to prefer when the vault holds both a service account key and an OAuth token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Delegation if a key is there, OAuth otherwise. Right almost always.
    #[default]
    Auto,
    /// Always the service account. Fails loudly rather than quietly falling back to a weaker
    /// credential — which matters when the difference is "reaches Gmail" or not.
    ServiceAccount,
    /// Always OAuth, even with a key present. For testing the other path, or when delegation
    /// has been revoked and you want the browser login back.
    Oauth,
}

impl Strategy {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().replace('-', "_").as_str() {
            "auto" => Some(Self::Auto),
            "service_account" | "sa" | "delegated" => Some(Self::ServiceAccount),
            "oauth" => Some(Self::Oauth),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ServiceAccount => "service_account",
            Self::Oauth => "oauth",
        }
    }
}

fn default_active() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub label: String,
    pub auth: AuthConfig,
    /// Present when this account authenticates as a service account impersonating a Workspace
    /// user. When it is set it WINS — `auth` then holds nothing but the cached access token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account: Option<DelegatedAccount>,
}

/// A service account key plus the user it acts as. Domain-wide delegation in one struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedAccount {
    /// The Workspace user being impersonated — whose mail, calendar and drive these calls touch.
    pub subject: String,
    /// The key file verbatim. Kept whole rather than split into fields so a rotated key is a
    /// straight replacement, and so nothing here has to understand the key format.
    pub key_json: String,
    /// Exactly the scopes the admin console authorised for this key. Asking for one that was not
    /// authorised fails the whole token request, so this list is the contract with the console.
    #[serde(default)]
    pub scopes: Vec<String>,
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

impl AuthConfig {
    /// The OAuth half of a delegated account: nothing but a slot for the cached access token.
    pub fn empty() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            token_expiry: Utc::now(),
        }
    }
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

    /// The restored vault file: `~/.config/secrets.env`, written by `vaulty secrets pull`.
    /// Overridable so a machine that keeps its environment somewhere else still restores — and so
    /// this path can be exercised without writing to the owner's real credentials file.
    fn secrets_env() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("VGOOG_SECRETS_ENV") {
            return Some(PathBuf::from(path));
        }
        Some(dirs::home_dir()?.join(".config").join("secrets.env"))
    }

    /// Rebuild a config from the vault when this machine has none of its own.
    ///
    /// vgoog owns its credentials; the api only keeps an encrypted backup. This is the path that
    /// makes the backup worth having — on a fresh machine, or after the config was blown away,
    /// `vaulty secrets pull` followed by any vgoog command is the whole recovery.
    fn from_secrets_env() -> Option<Self> {
        let body = std::fs::read_to_string(Self::secrets_env()?).ok()?;

        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for line in body.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            // The file is sourced by the shell, so entries read `export NAME=value`.
            let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
            let Some((name, value)) = line.split_once('=') else { continue };
            let value = value.trim().trim_matches('"').trim_matches('\'');
            values.insert(name.trim().to_string(), value.to_string());
        }

        let take = |key: &str| values.get(key).filter(|value| !value.is_empty()).cloned();

        // A delegated key beats an OAuth token when both are present: it cannot be revoked by a
        // password change, it needs no browser, and it reaches more. One key, one account per
        // subject — which is how "several Google accounts" arrives on a fresh machine with no
        // interaction at all. VGOOG_STRATEGY overrides that preference either way.
        let strategy = take("VGOOG_STRATEGY").and_then(|value| Strategy::parse(&value)).unwrap_or_default();

        if strategy != Strategy::Oauth {
            if let Some(key_json) = take("VGOOG_SERVICE_ACCOUNT_KEY") {
                let subjects = take("VGOOG_SUBJECTS").unwrap_or_default();
                let subjects: Vec<String> = subjects
                    .split(',')
                    .map(|subject| subject.trim().to_string())
                    .filter(|subject| !subject.is_empty())
                    .collect();

                if !subjects.is_empty() && ServiceAccountKey::parse(&key_json).is_ok() {
                    let scopes: Vec<String> = crate::auth::scopes::all().iter().map(|s| s.to_string()).collect();
                    let mut accounts = BTreeMap::new();

                    for subject in &subjects {
                        // The local part is the account name: uri@maxima.ninja → "uri".
                        let name = subject.split('@').next().unwrap_or(subject).to_lowercase();
                        accounts.insert(
                            name,
                            Account {
                                label: subject.clone(),
                                auth: AuthConfig::empty(),
                                service_account: Some(DelegatedAccount {
                                    subject: subject.clone(),
                                    key_json: key_json.clone(),
                                    scopes: scopes.clone(),
                                }),
                            },
                        );
                    }

                    // The FIRST address listed is the primary one — accounts are a BTreeMap, so taking
                    // its first key would silently hand the session to whoever sorts alphabetically.
                    let active = subjects
                        .first()
                        .map(|subject| subject.split('@').next().unwrap_or(subject).to_lowercase())
                        .unwrap_or_else(default_active);
                    return Some(Config { accounts, active_account: active, auth: None, strategy });
                }
                }
            }

        // Asked for delegation and there is no usable key — say so rather than silently handing
        // back an OAuth account that cannot do what was asked for.
        if strategy == Strategy::ServiceAccount {
            return None;
        }

        let auth = AuthConfig {
            client_id: take("VGOOG_CLIENT_ID")?,
            client_secret: take("VGOOG_CLIENT_SECRET")?,
            // Access tokens are minted from the refresh token on the next call; an empty one here
            // just means the first request refreshes before it runs.
            access_token: String::new(),
            refresh_token: take("VGOOG_REFRESH_TOKEN")?,
            token_expiry: Utc::now(),
        };

        let mut accounts = BTreeMap::new();
        accounts.insert(
            "default".to_string(),
            Account { label: "Restored from vault".to_string(), auth, service_account: None },
        );

        Some(Config { accounts, active_account: "default".to_string(), auth: None, strategy })
    }

    /// Restore from the vault, and write it down — from here on this machine has its own copy.
    fn restore() -> Option<Self> {
        let restored = Self::from_secrets_env()?;
        let _ = restored.save();
        Some(restored)
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            if let Some(restored) = Self::restore() {
                return Ok(restored);
            }
            return Err(VgoogError::Config(
                "Config not found. Run `vgoog` to set up, or `vaulty secrets pull` to restore from the vault.".into(),
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
                        service_account: None,
                    },
                );
                config.active_account = "default".to_string();
                // Save migrated config
                let _ = config.save();
            }
        }

        // A config file that EXISTS but holds nothing usable is the same as no config at all —
        // and it is the common case, because an abandoned setup leaves empty strings behind. Only
        // checking `path.exists()` meant a machine with credentials sitting in the vault kept
        // reporting "missing refresh_token" forever.
        if !config.has_credentials() {
            if let Some(restored) = Self::restore() {
                return Ok(restored);
            }
        }

        Ok(config)
    }

    /// Is there an account here that could actually talk to Google? An access token is not
    /// required — it is minted from the refresh token on the next call.
    fn has_credentials(&self) -> bool {
        self.accounts.values().any(account_is_usable)
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

    /// True when vgoog can start without asking anything — its own config, or a vault file it can
    /// restore from. Without the second half, a restored machine walks into the setup wizard while
    /// working credentials sit unread on disk.
    pub fn exists() -> bool {
        let own = Self::load().map(|config| config.has_credentials()).unwrap_or(false);
        own || Self::from_secrets_env().is_some()
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
        self.active().is_ok_and(account_is_usable)
    }

    /// Build a legacy-compatible Config for a single account (used by GoogleClient)
    pub fn for_active_account(&self) -> Result<SingleAccountConfig> {
        let account = self.active()?;
        Ok(SingleAccountConfig {
            auth: account.auth.clone(),
            service_account: account.service_account.clone(),
            config_ref: self.clone(),
        })
    }
}

/// A view of a single account's config, used by GoogleClient
#[derive(Debug, Clone)]
pub struct SingleAccountConfig {
    pub auth: AuthConfig,
    pub service_account: Option<DelegatedAccount>,
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

/// Can this account get a token at all?
///
/// The two credential kinds have nothing in common: a delegated account holds a key and no refresh
/// token, an OAuth account holds a refresh token and no key. Asking "is the refresh token set"
/// declared every service account broken.
pub fn account_is_usable(account: &Account) -> bool {
    if let Some(delegated) = &account.service_account {
        return !delegated.subject.is_empty() && !delegated.key_json.is_empty();
    }
    !account.auth.client_id.is_empty()
        && !account.auth.client_secret.is_empty()
        && !account.auth.refresh_token.is_empty()
}
