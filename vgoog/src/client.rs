use crate::auth::ensure_token;
use crate::config::{Config, SingleAccountConfig};
use crate::error::{Result, VgoogError};
use crate::tier::{self, Tier};
use reqwest::{Client, Method, Response};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GoogleClient {
    http: Client,
    /// The account this client acts as — not necessarily the active one. `--account` builds a
    /// client for any account and leaves `active_account` alone.
    account_config: Arc<Mutex<SingleAccountConfig>>,
    /// The full multi-account config for account switching
    full_config: Arc<Mutex<Config>>,
}

impl GoogleClient {
    /// A client for the named account, or for the active one when `account` is None.
    pub fn new(config: Config, account: Option<&str>) -> Result<Self> {
        let name = account.map(str::to_string).unwrap_or_else(|| config.active_account.clone());
        let account_config = config.for_account(&name)?;
        let http = Client::builder()
            .user_agent("vgoog/0.1.0")
            .build()
            .expect("Failed to create HTTP client");
        Ok(Self {
            http,
            account_config: Arc::new(Mutex::new(account_config)),
            full_config: Arc::new(Mutex::new(config)),
        })
    }

    /// A client for another account that shares this one's connections. The active account does
    /// not change.
    pub async fn for_account(&self, name: &str) -> Result<Self> {
        let account_config = self.full_config.lock().await.for_account(name)?;
        Ok(Self {
            http: self.http.clone(),
            account_config: Arc::new(Mutex::new(account_config)),
            full_config: self.full_config.clone(),
        })
    }

    /// Make another account the active one — the TUI's Ctrl+A. Only `active_account` is written,
    /// so a token another vgoog process saved in the meantime survives.
    pub async fn switch_account(&self, name: &str) -> Result<()> {
        let mut full = self.full_config.lock().await;
        let account_config = full.for_account(name)?;
        Config::update(&full, |config| config.active_account = name.to_string())?;
        full.active_account = name.to_string();
        *self.account_config.lock().await = account_config;
        Ok(())
    }

    /// The name of the account this client acts as
    pub async fn active_account_name(&self) -> String {
        self.account_config.lock().await.name.clone()
    }

    /// The label of the account this client acts as
    pub async fn active_account_label(&self) -> String {
        let name = self.active_account_name().await;
        let full = self.full_config.lock().await;
        full.accounts
            .get(&name)
            .map(|a| a.label.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// The tier of the account this client acts as
    pub async fn tier(&self) -> Tier {
        self.account_config.lock().await.tier
    }

    /// Get all account names
    pub async fn account_names(&self) -> Vec<String> {
        self.full_config
            .lock()
            .await
            .account_names()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get the full config (for account management)
    pub async fn get_full_config(&self) -> Config {
        self.full_config.lock().await.clone()
    }

    /// Update the full config (after adding/removing accounts)
    pub async fn update_full_config(&self, config: Config) -> Result<()> {
        config.save()?;
        let new_account_config = config.for_active_account()?;
        let mut full = self.full_config.lock().await;
        *full = config;
        let mut current = self.account_config.lock().await;
        *current = new_account_config;
        Ok(())
    }

    /// The one door every request goes through: refuse what this account's tier does not allow,
    /// then hand back a token for the rest. The refusal comes first, so a denied call never even
    /// mints a token.
    async fn authorize(&self, url: &str) -> Result<String> {
        let mut config = self.account_config.lock().await;
        tier::check(&config.name, config.tier, &config.scopes, url)?;
        ensure_token(&mut config).await?;
        Ok(config.auth.access_token.clone())
    }

    async fn handle_response(&self, resp: Response) -> Result<Value> {
        let status = resp.status();
        if status.is_success() {
            if status == reqwest::StatusCode::NO_CONTENT {
                return Ok(Value::Null);
            }
            let body = resp.text().await?;
            if body.is_empty() {
                return Ok(Value::Null);
            }
            let val: Value = serde_json::from_str(&body)?;
            Ok(val)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            Err(VgoogError::RateLimited {
                retry_after_secs: retry_after,
            })
        } else if status == reqwest::StatusCode::NOT_FOUND {
            Err(VgoogError::NotFound("Resource not found".into()))
        } else {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(VgoogError::Api {
                status: code,
                message: body,
            })
        }
    }

    pub async fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let token = self.authorize(url).await?;
        let mut req = self.http.request(method, url).bearer_auth(&token);
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?;
        self.handle_response(resp).await
    }

    pub async fn get(&self, url: &str) -> Result<Value> {
        self.request(Method::GET, url, None).await
    }

    pub async fn post(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::POST, url, Some(body)).await
    }

    pub async fn put(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::PUT, url, Some(body)).await
    }

    pub async fn patch(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::PATCH, url, Some(body)).await
    }

    pub async fn delete(&self, url: &str) -> Result<Value> {
        self.request(Method::DELETE, url, None).await
    }

    pub async fn upload_multipart(
        &self,
        url: &str,
        metadata: &Value,
        file_bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<Value> {
        let token = self.authorize(url).await?;
        let metadata_part = reqwest::multipart::Part::text(serde_json::to_string(metadata)?)
            .mime_str("application/json")?;
        let file_part = reqwest::multipart::Part::bytes(file_bytes).mime_str(mime_type)?;
        let form = reqwest::multipart::Form::new()
            .part("metadata", metadata_part)
            .part("file", file_part);

        let resp = self
            .http
            .post(url)
            .bearer_auth(&token)
            .multipart(form)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    pub async fn download(&self, url: &str) -> Result<Vec<u8>> {
        let token = self.authorize(url).await?;
        let resp = self.http.get(url).bearer_auth(&token).send().await?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp.bytes().await?.to_vec())
        } else {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(VgoogError::Api {
                status: code,
                message: body,
            })
        }
    }

    pub async fn post_empty(&self, url: &str) -> Result<Value> {
        let token = self.authorize(url).await?;
        let resp = self.http.post(url).bearer_auth(&token).send().await?;
        self.handle_response(resp).await
    }
}
