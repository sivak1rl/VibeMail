use crate::auth::{keychain, oauth};
use crate::db::models::Account;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tracing::debug;

/// Cached token with expiry.
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

/// In-memory token cache keyed by account ID.
/// Avoids refreshing on every IMAP/SMTP connect.
static TOKEN_CACHE: OnceLock<Mutex<HashMap<String, CachedToken>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, CachedToken>> {
    TOKEN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// How long to cache an access token before refreshing.
/// OAuth access tokens typically last 1 hour; we refresh at 50 minutes.
const CACHE_TTL_SECS: u64 = 50 * 60;

/// Get a valid access token for the account, using cache when possible.
///
/// For OAuth accounts: checks in-memory cache first, then refreshes via the
/// provider's token endpoint. Detects token revocation (invalid_grant) and
/// returns a clear "re-auth required" error.
///
/// For password accounts: returns the stored password from keychain.
pub async fn get_access_token(account: &Account) -> Result<String> {
    // Password accounts don't use OAuth tokens
    if account.provider != "gmail" && account.provider != "outlook" {
        return keychain::get_token(&account.id, "password")?
            .ok_or_else(|| anyhow!("No password for account {}", account.email));
    }

    // Check cache
    {
        let cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = cache.get(&account.id) {
            if cached.expires_at > Instant::now() {
                debug!("Using cached access token for {}", account.email);
                return Ok(cached.access_token.clone());
            }
        }
    }

    // Cache miss or expired — refresh
    let token = refresh_access_token(account).await?;

    // Store in cache
    {
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(
            account.id.clone(),
            CachedToken {
                access_token: token.clone(),
                expires_at: Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS),
            },
        );
    }

    Ok(token)
}

/// Invalidate the cached token for an account (e.g., after auth failure).
pub fn invalidate(account_id: &str) {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.remove(account_id);
}

/// Refresh the access token via the OAuth provider.
/// Detects revocation and returns a specific error message.
async fn refresh_access_token(account: &Account) -> Result<String> {
    let refresh = keychain::get_token(&account.id, "refresh_token")?;
    if refresh.is_none() {
        // No refresh token — try the stored access token as a fallback
        if let Some(token) = keychain::get_token(&account.id, "access_token")? {
            return Ok(token);
        }
        return Err(anyhow!("No tokens for {}; re-auth required", account.email));
    }
    let refresh = refresh.unwrap();

    let client_id = keychain::get_token(&account.id, "client_id")?
        .ok_or_else(|| anyhow!("No client_id for account {}", account.id))?;
    let client_secret = keychain::get_token(&account.id, "client_secret")?;

    let config = match account.provider.as_str() {
        "gmail" => oauth::OAuthConfig::gmail(&client_id, client_secret.as_deref()),
        "outlook" => oauth::OAuthConfig::outlook(&client_id, client_secret.as_deref()),
        _ => return Err(anyhow!("Unknown OAuth provider")),
    };

    match oauth::refresh_token(&config, &refresh).await {
        Ok(tokens) => {
            keychain::store_token(&account.id, "access_token", &tokens.access_token)?;
            if let Some(rt) = &tokens.refresh_token {
                keychain::store_token(&account.id, "refresh_token", rt)?;
            }
            Ok(tokens.access_token)
        }
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            // Detect token revocation from OAuth error responses
            if msg.contains("invalid_grant")
                || msg.contains("token has been expired or revoked")
                || msg.contains("token has been revoked")
                || msg.contains("401")
            {
                // Clear stale tokens so the user gets prompted to re-auth
                let _ = keychain::store_token(&account.id, "access_token", "");
                Err(anyhow!(
                    "Token revoked for {}; re-auth required. Please remove and re-add this account.",
                    account.email
                ))
            } else {
                Err(e)
            }
        }
    }
}
