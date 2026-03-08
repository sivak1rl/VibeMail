use crate::auth::{keychain, oauth};
use crate::db::{models::Account, Database};
use crate::{OAuthFlow, OAuthPending};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddAccountRequest {
    pub name: String,
    pub email: String,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub password: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthUrlResponse {
    pub url: String,
    pub account_id: String,
    // code_verifier and state stored server-side in keychain during the flow
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteOAuthRequest {
    pub account_id: String,
    pub code: String,
    pub code_verifier: String,
    pub provider: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

/// Start OAuth: build URL, store PKCE state, spawn the redirect listener, return URL.
/// The listener binds port 7887 synchronously before this command returns, so the
/// browser redirect will always find an open socket.
#[tauri::command]
pub async fn get_oauth_url(
    provider: String,
    client_id: String,
    client_secret: Option<String>,
    pending: State<'_, OAuthPending>,
) -> Result<OAuthUrlResponse, String> {
    let account_id = Uuid::new_v4().to_string();

    let config =
        build_config(&provider, &client_id, client_secret.as_deref()).map_err(|e| e.to_string())?;

    let oauth_state = oauth::build_auth_url(&config);

    // Bind the listener NOW (before the browser opens) so port 7887 is ready.
    let listener = oauth::bind_redirect_listener()
        .await
        .map_err(|e| e.to_string())?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<oauth::OAuthCallback, String>>();
    let expected_state = oauth_state.state.clone();
    tokio::spawn(async move {
        let result = oauth::accept_redirect(listener, &expected_state)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });

    // Store all ephemeral OAuth state in memory — no keychain needed for these
    pending.lock().await.insert(
        account_id.clone(),
        OAuthFlow {
            rx,
            code_verifier: oauth_state.code_verifier,
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
            provider: provider.clone(),
        },
    );

    Ok(OAuthUrlResponse {
        url: oauth_state.url,
        account_id,
    })
}

/// Wait for the browser to hit the local redirect server and auto-complete OAuth.
/// Returns the account that was created.
#[tauri::command]
pub async fn await_oauth_redirect(
    account_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
    pending: State<'_, OAuthPending>,
) -> Result<Account, String> {
    // Pull all ephemeral state that was stored in memory by get_oauth_url
    let flow = pending
        .lock()
        .await
        .remove(&account_id)
        .ok_or("No pending OAuth for this account — call get_oauth_url first")?;

    // Wait for the spawned listener task to deliver the callback
    let callback = flow
        .rx
        .await
        .map_err(|_| "OAuth listener task was dropped".to_string())??;

    let code_verifier = flow.code_verifier;
    let client_id = flow.client_id;
    let client_secret = flow.client_secret;
    let provider = flow.provider;

    // Exchange code for tokens
    let config =
        build_config(&provider, &client_id, client_secret.as_deref()).map_err(|e| e.to_string())?;

    let tokens = oauth::exchange_code(&config, &callback.code, &code_verifier)
        .await
        .map_err(|e| e.to_string())?;

    keychain::store_token(&account_id, "access_token", &tokens.access_token)
        .map_err(|e| e.to_string())?;
    if let Some(refresh) = &tokens.refresh_token {
        keychain::store_token(&account_id, "refresh_token", refresh).map_err(|e| e.to_string())?;
    }
    // Persist client credentials for token refresh
    keychain::store_token(&account_id, "client_id", &client_id).map_err(|e| e.to_string())?;
    if let Some(secret) = &client_secret {
        keychain::store_token(&account_id, "client_secret", secret).map_err(|e| e.to_string())?;
    }

    let (imap_host, imap_port, smtp_host, smtp_port) = provider_endpoints(&provider);

    // Fetch the user's email address from the provider's userinfo endpoint
    let email = fetch_user_email(&tokens.access_token, &provider)
        .await
        .map_err(|e| e.to_string())?;

    let name = email
        .split('@')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("Account")
        .to_string();

    let account = Account {
        id: account_id.clone(),
        name,
        email,
        provider,
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
    };

    let db = db.lock().await;
    db.upsert_account(&account).map_err(|e| e.to_string())?;
    Ok(account)
}

/// Fallback: complete OAuth manually (user pastes code from URL)
#[tauri::command]
pub async fn complete_oauth(
    request: CompleteOAuthRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Account, String> {
    let config = build_config(
        &request.provider,
        &request.client_id,
        request.client_secret.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    let tokens = oauth::exchange_code(&config, &request.code, &request.code_verifier)
        .await
        .map_err(|e| e.to_string())?;

    keychain::store_token(&request.account_id, "access_token", &tokens.access_token)
        .map_err(|e| e.to_string())?;
    if let Some(refresh) = &tokens.refresh_token {
        keychain::store_token(&request.account_id, "refresh_token", refresh)
            .map_err(|e| e.to_string())?;
    }

    let email = fetch_user_email(&tokens.access_token, &request.provider)
        .await
        .unwrap_or_default();

    let (imap_host, imap_port, smtp_host, smtp_port) = provider_endpoints(&request.provider);

    let account = Account {
        id: request.account_id,
        name: email.split('@').next().unwrap_or("Account").to_string(),
        email,
        provider: request.provider,
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
    };

    let db = db.lock().await;
    db.upsert_account(&account).map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
pub async fn add_account(
    request: AddAccountRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Account, String> {
    let account = Account {
        id: Uuid::new_v4().to_string(),
        name: request.name,
        email: request.email.clone(),
        provider: request.provider.clone(),
        imap_host: request.imap_host.unwrap_or_else(|| "localhost".to_string()),
        imap_port: request.imap_port.unwrap_or(993),
        smtp_host: request.smtp_host.unwrap_or_else(|| "localhost".to_string()),
        smtp_port: request.smtp_port.unwrap_or(587),
    };

    if let Some(password) = &request.password {
        keychain::store_token(&account.id, "password", password).map_err(|e| e.to_string())?;
    }
    if let Some(cid) = &request.client_id {
        keychain::store_token(&account.id, "client_id", cid).map_err(|e| e.to_string())?;
    }
    if let Some(secret) = &request.client_secret {
        keychain::store_token(&account.id, "client_secret", secret).map_err(|e| e.to_string())?;
    }

    let db = db.lock().await;
    db.upsert_account(&account).map_err(|e| e.to_string())?;
    Ok(account)
}

#[tauri::command]
pub async fn list_accounts(db: State<'_, Arc<Mutex<Database>>>) -> Result<Vec<Account>, String> {
    let db = db.lock().await;
    db.list_accounts().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_account(
    account_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    for key in &[
        "access_token",
        "refresh_token",
        "password",
        "client_id",
        "client_secret",
    ] {
        let _ = keychain::delete_token(&account_id, key);
    }
    let db = db.lock().await;
    db.delete_account(&account_id).map_err(|e| e.to_string())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn build_config(
    provider: &str,
    client_id: &str,
    client_secret: Option<&str>,
) -> anyhow::Result<oauth::OAuthConfig> {
    match provider {
        "gmail" => Ok(oauth::OAuthConfig::gmail(client_id, client_secret)),
        "outlook" => Ok(oauth::OAuthConfig::outlook(client_id, client_secret)),
        other => Err(anyhow::anyhow!("Unknown provider: {}", other)),
    }
}

fn provider_endpoints(provider: &str) -> (String, u16, String, u16) {
    match provider {
        "gmail" => ("imap.gmail.com".into(), 993, "smtp.gmail.com".into(), 587),
        "outlook" => (
            "outlook.office365.com".into(),
            993,
            "smtp.office365.com".into(),
            587,
        ),
        _ => ("localhost".into(), 993, "localhost".into(), 587),
    }
}

/// Fetch the authenticated user's email address from the provider's userinfo endpoint.
async fn fetch_user_email(access_token: &str, provider: &str) -> anyhow::Result<String> {
    let userinfo_url = match provider {
        "gmail" => "https://www.googleapis.com/oauth2/v3/userinfo",
        "outlook" => "https://graph.microsoft.com/v1.0/me",
        _ => return Err(anyhow::anyhow!("No userinfo endpoint for provider")),
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    let json: serde_json::Value = resp.json().await?;

    let email = match provider {
        "gmail" => json["email"].as_str().unwrap_or("").to_string(),
        "outlook" => json["mail"]
            .as_str()
            .or_else(|| json["userPrincipalName"].as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };

    Ok(email)
}
