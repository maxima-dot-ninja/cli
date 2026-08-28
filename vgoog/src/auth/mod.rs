pub mod callback;
pub mod oauth;
pub mod scopes;
pub mod service_account;

use crate::config::SingleAccountConfig;
use crate::error::Result;

/// Make sure the account has a usable access token, whichever way it authenticates.
///
/// Two paths, one door. The client never needs to know which kind of credential is behind an
/// account — it asks for a token and gets one:
///
///   - **service account** — mints a fresh assertion and exchanges it. Nothing is stored between
///     calls but the key, and nothing can be revoked out from under it except the key itself.
///   - **oauth** — refreshes the stored refresh token when the access token is close to expiry.
///
/// Returns true when a new token was fetched.
pub async fn ensure_token(config: &mut SingleAccountConfig) -> Result<bool> {
    match config.service_account.clone() {
        Some(delegated) => oauth::refresh_service_account(config, &delegated).await,
        None => oauth::refresh_token_if_needed(config).await,
    }
}
