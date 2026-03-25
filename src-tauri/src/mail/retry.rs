use anyhow::{anyhow, Result};
use std::time::Duration;
use tracing::warn;

/// Maximum number of connection attempts before giving up.
const MAX_ATTEMPTS: u32 = 3;

/// Initial backoff delay between retries.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Determines whether an error is likely transient and worth retrying.
/// Returns false for permanent failures (auth, config, token revocation).
fn is_retryable(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();

    // Permanent failures — never retry
    if msg.contains("re-auth required")
        || msg.contains("token revoked")
        || msg.contains("invalid_grant")
        || msg.contains("no password")
        || msg.contains("no client_id")
        || msg.contains("unknown oauth provider")
        || msg.contains("auth failed")
        || msg.contains("login failed")
        || msg.contains("invalid from address")
        || msg.contains("invalid to address")
    {
        return false;
    }

    // Transient failures — retry
    if msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("connection reset")
        || msg.contains("connection refused")
        || msg.contains("broken pipe")
        || msg.contains("dns")
        || msg.contains("network")
        || msg.contains("temporarily unavailable")
        || msg.contains("try again")
        || msg.contains("eof")
        || msg.contains("connection closed")
    {
        return true;
    }

    // Default: don't retry unknown errors
    false
}

/// Execute an async operation with retry and exponential backoff.
///
/// Retries up to MAX_ATTEMPTS times for transient errors.
/// Permanent errors (auth failures, missing config) fail immediately.
pub async fn with_retry<F, Fut, T>(operation: &str, f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = anyhow!("{} failed", operation);
    let mut backoff = INITIAL_BACKOFF;

    for attempt in 1..=MAX_ATTEMPTS {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if !is_retryable(&e) || attempt == MAX_ATTEMPTS {
                    if attempt > 1 {
                        warn!("{} failed after {} attempts: {}", operation, attempt, e);
                    }
                    return Err(e);
                }

                warn!(
                    "{} attempt {}/{} failed (retrying in {:?}): {}",
                    operation, attempt, MAX_ATTEMPTS, backoff, e
                );
                tokio::time::sleep(backoff).await;
                backoff *= 2;
                last_err = e;
            }
        }
    }

    Err(last_err)
}
