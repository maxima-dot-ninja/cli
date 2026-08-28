// Service account auth: domain-wide delegation.
//
// This is the deeper of the two paths, and the one with no moving parts at runtime. Instead of
// holding a refresh token that a human had to fetch through a browser, vgoog holds a private key
// and MINTS its own assertion for every token: a short-lived JWT saying "I am this service
// account, acting as this user, for these scopes". Google trades it for an access token.
//
// What that buys over OAuth:
//   - nothing to re-authorise; a key does not expire the way a refresh token can be revoked
//   - no browser, no callback, no redirect URI, so it works headless and in CI
//   - reaches Workspace-admin surfaces (directory, groups) that a user-consent flow cannot
//
// The cost is that it is Workspace-only, and an admin has to authorise the service account's
// client id against an exact scope list in the admin console. Miss a scope there and the call
// fails with `unauthorized_client` — which is why scopes.rs is the single source for both.

use crate::error::{Result, VgoogError};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// Google caps assertion lifetime at an hour; we never need it to outlive the exchange itself.
const ASSERTION_TTL_SECS: i64 = 3600;

/// The JSON key file downloaded from the Google Cloud console. Only the fields we use are read,
/// so a key with extra fields (or a newer format) still loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountKey {
    pub client_email: String,
    pub private_key: String,
    #[serde(default)]
    pub private_key_id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub client_id: String,
}

impl ServiceAccountKey {
    pub fn parse(json: &str) -> Result<Self> {
        let key: Self = serde_json::from_str(json)
            .map_err(|error| VgoogError::Config(format!("not a service account key: {error}")))?;
        if key.client_email.is_empty() || key.private_key.is_empty() {
            return Err(VgoogError::Config("service account key is missing client_email or private_key".into()));
        }
        Ok(key)
    }
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: String,
    aud: &'a str,
    exp: i64,
    iat: i64,
    /// The user being impersonated. This is what domain-wide delegation authorises, and what
    /// makes "the service account's Gmail" mean the owner's actual inbox.
    #[serde(skip_serializing_if = "str::is_empty")]
    sub: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn assertion(key: &ServiceAccountKey, subject: &str, scopes: &[&str]) -> Result<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        iss: &key.client_email,
        scope: scopes.join(" "),
        aud: TOKEN_URL,
        exp: now + ASSERTION_TTL_SECS,
        iat: now,
        sub: subject,
    };

    let mut header = Header::new(Algorithm::RS256);
    // Not required by Google, but it makes a key-rotation mistake identifiable from the token.
    if !key.private_key_id.is_empty() {
        header.kid = Some(key.private_key_id.clone());
    }

    let encoding = EncodingKey::from_rsa_pem(key.private_key.as_bytes())
        .map_err(|error| VgoogError::Auth(format!("service account private key is unreadable: {error}")))?;

    jsonwebtoken::encode(&header, &claims, &encoding)
        .map_err(|error| VgoogError::Auth(format!("could not sign the assertion: {error}")))
}

/// Trade a signed assertion for an access token. Returns the token and when it expires.
pub async fn fetch_token(
    key: &ServiceAccountKey,
    subject: &str,
    scopes: &[&str],
) -> Result<(String, chrono::DateTime<Utc>)> {
    let assertion = assertion(key, subject, scopes)?;

    let response = reqwest::Client::new()
        .post(TOKEN_URL)
        .form(&[("grant_type", GRANT_TYPE), ("assertion", assertion.as_str())])
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        // Google's two usual complaints here have precise causes, and guessing at them has cost
        // enough time already — say what each one actually means.
        if let Ok(error) = serde_json::from_str::<TokenError>(&body) {
            let detail = error.error_description.unwrap_or_default();
            let hint = match error.error.as_str() {
                "unauthorized_client" => "\n  → the admin console has not authorised this service account's client id for these scopes (Security → API controls → Domain-wide delegation)",
                "invalid_grant" => "\n  → domain-wide delegation is off for this key, or the impersonated user is not in this Workspace",
                _ => "",
            };
            return Err(VgoogError::Auth(format!("{}: {detail}{hint}", error.error)));
        }
        return Err(VgoogError::Api { status, message: body });
    }

    let token: TokenResponse = response.json().await?;
    let expiry = Utc::now() + chrono::Duration::seconds(token.expires_in);
    Ok((token.access_token, expiry))
}
