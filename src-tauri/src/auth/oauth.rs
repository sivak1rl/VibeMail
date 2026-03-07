use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::info;
use url::Url;

pub const REDIRECT_PORT: u16 = 7887;
pub const REDIRECT_URI: &str = "http://localhost:7887/oauth/callback";

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub provider: OAuthProvider,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OAuthProvider {
    Gmail,
    Outlook,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub url: String,
    pub state: String,
    pub code_verifier: String,
    pub provider: String,
}

/// Code + state captured from the redirect
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
}

impl OAuthConfig {
    pub fn gmail(client_id: &str, client_secret: Option<&str>) -> Self {
        Self {
            provider: OAuthProvider::Gmail,
            client_id: client_id.to_string(),
            client_secret: client_secret.map(|s| s.to_string()),
            redirect_uri: REDIRECT_URI.to_string(),
        }
    }

    pub fn outlook(client_id: &str, client_secret: Option<&str>) -> Self {
        Self {
            provider: OAuthProvider::Outlook,
            client_id: client_id.to_string(),
            client_secret: client_secret.map(|s| s.to_string()),
            redirect_uri: REDIRECT_URI.to_string(),
        }
    }
}

fn generate_pkce() -> (String, String) {
    let mut verifier_bytes = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn build_auth_url(config: &OAuthConfig) -> OAuthState {
    let (code_verifier, code_challenge) = generate_pkce();
    let state = generate_state();

    let (auth_endpoint, scopes) = match config.provider {
        OAuthProvider::Gmail => (
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://mail.google.com/ email profile",
        ),
        OAuthProvider::Outlook => (
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access email",
        ),
    };

    let mut url = Url::parse(auth_endpoint).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("scope", scopes)
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    let provider_name = match config.provider {
        OAuthProvider::Gmail => "gmail",
        OAuthProvider::Outlook => "outlook",
    };

    OAuthState {
        url: url.to_string(),
        state,
        code_verifier,
        provider: provider_name.to_string(),
    }
}

/// Bind the redirect listener on port 7887. Call this before opening the browser
/// so the socket is ready by the time Google/Microsoft redirects back.
pub async fn bind_redirect_listener() -> Result<TcpListener> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT))
        .await
        .map_err(|e| anyhow!("Could not listen on port {}: {}", REDIRECT_PORT, e))?;
    info!("OAuth redirect listener bound on port {}", REDIRECT_PORT);
    Ok(listener)
}

/// Accept one connection on an already-bound listener and parse the OAuth callback.
pub async fn accept_redirect(listener: TcpListener, expected_state: &str) -> Result<OAuthCallback> {
    let (mut stream, _) = listener.accept().await?;

    // Read the HTTP request
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the GET line: "GET /oauth/callback?code=...&state=... HTTP/1.1"
    let callback = parse_callback_from_request(&request)?;

    // Validate CSRF state
    if callback.state != expected_state {
        let body = "<h2>Error: state mismatch. Please try again.</h2>";
        let _ = send_html_response(&mut stream, 400, body).await;
        return Err(anyhow!("OAuth state mismatch — possible CSRF"));
    }

    // Send success page that auto-closes
    let success_html = r#"<!DOCTYPE html>
<html><head><title>VibeMail — Connected</title>
<style>body{font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#1a1a1a;color:#e8e8e8;}
.card{background:#242424;border:1px solid #383838;border-radius:12px;padding:40px;text-align:center;max-width:360px;}
h2{color:#4f8ef7;margin:0 0 12px;}p{color:#888;margin:0;}</style>
</head><body><div class="card">
<h2>&#10003; Connected!</h2>
<p>You can close this window and return to VibeMail.</p>
</div><script>setTimeout(()=>window.close(),2000)</script></body></html>"#;

    let _ = send_html_response(&mut stream, 200, success_html).await;
    info!("OAuth callback received successfully");
    Ok(callback)
}

fn parse_callback_from_request(request: &str) -> Result<OAuthCallback> {
    // First line: "GET /oauth/callback?foo=bar HTTP/1.1"
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("Malformed HTTP request"))?;

    let url = Url::parse(&format!("http://localhost{}", path))?;
    let params: HashMap<_, _> = url.query_pairs().collect();

    if let Some(error) = params.get("error") {
        return Err(anyhow!("OAuth error: {}", error));
    }

    let code = params
        .get("code")
        .ok_or_else(|| anyhow!("No 'code' in OAuth callback"))?
        .to_string();
    let state = params
        .get("state")
        .ok_or_else(|| anyhow!("No 'state' in OAuth callback"))?
        .to_string();

    Ok(OAuthCallback { code, state })
}

async fn send_html_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> Result<()> {
    let status_text = if status == 200 { "OK" } else { "Bad Request" };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, status_text, body.len(), body
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

pub async fn exchange_code(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse> {
    let token_endpoint = match config.provider {
        OAuthProvider::Gmail => "https://oauth2.googleapis.com/token",
        OAuthProvider::Outlook => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    };

    let client = reqwest::Client::new();
    let mut params = HashMap::new();
    params.insert("client_id", config.client_id.as_str());
    params.insert("code", code);
    params.insert("redirect_uri", config.redirect_uri.as_str());
    params.insert("grant_type", "authorization_code");
    params.insert("code_verifier", code_verifier);

    // Google and Microsoft both require client_secret for desktop app flows
    let secret_owned;
    if let Some(secret) = &config.client_secret {
        secret_owned = secret.clone();
        params.insert("client_secret", secret_owned.as_str());
    }

    let resp = client.post(token_endpoint).form(&params).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Token exchange failed ({status}): {body}"));
    }

    let token: TokenResponse = resp.json().await?;
    info!("OAuth token exchange successful");
    Ok(token)
}

pub async fn refresh_token(config: &OAuthConfig, refresh_tok: &str) -> Result<TokenResponse> {
    let token_endpoint = match config.provider {
        OAuthProvider::Gmail => "https://oauth2.googleapis.com/token",
        OAuthProvider::Outlook => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    };

    let client = reqwest::Client::new();
    let mut params = HashMap::new();
    params.insert("client_id", config.client_id.as_str());
    params.insert("refresh_token", refresh_tok);
    params.insert("grant_type", "refresh_token");

    let secret_owned;
    if let Some(secret) = &config.client_secret {
        secret_owned = secret.clone();
        params.insert("client_secret", secret_owned.as_str());
    }

    let resp = client
        .post(token_endpoint)
        .form(&params)
        .send()
        .await?
        .error_for_status()?;

    let token: TokenResponse = resp.json().await?;
    Ok(token)
}

/// Build raw XOAUTH2 SASL string for IMAP authentication.
/// Returns the raw (unencoded) SASL string — async-imap's Authenticator handles base64.
pub fn build_xoauth2(email: &str, access_token: &str) -> String {
    format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token)
}
