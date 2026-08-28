use crate::config::SingleAccountConfig;
use crate::error::{Result, VgoogError};
use chrono::{Duration, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: String,
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Refresh access token if expired (2-minute buffer)
pub async fn refresh_token_if_needed(config: &mut SingleAccountConfig) -> Result<bool> {
    if Utc::now() < config.auth.token_expiry - Duration::minutes(2) {
        return Ok(false);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", config.auth.client_id.as_str()),
            ("client_secret", config.auth.client_secret.as_str()),
            ("refresh_token", config.auth.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<TokenErrorResponse>(&body) {
            return Err(VgoogError::Auth(format!(
                "{}: {}",
                err.error,
                err.error_description.unwrap_or_default()
            )));
        }
        return Err(VgoogError::Api {
            status,
            message: body,
        });
    }

    let token: TokenResponse = resp.json().await?;
    config.auth.access_token = token.access_token;
    config.auth.token_expiry = Utc::now() + Duration::seconds(token.expires_in);
    if let Some(rt) = token.refresh_token {
        config.auth.refresh_token = rt;
    }
    config.save()?;

    Ok(true)
}
