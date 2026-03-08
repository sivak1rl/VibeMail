use crate::auth::{keychain, oauth};
use crate::db::models::{Account, ComposeMessage};
use anyhow::{anyhow, Result};
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::{Credentials, Mechanism},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::info;

pub async fn send_message(account: &Account, compose: &ComposeMessage) -> Result<()> {
    let from_mailbox: Mailbox = format!("{} <{}>", account.name, account.email)
        .parse()
        .map_err(|e| anyhow!("Invalid from address: {}", e))?;

    let mut builder = Message::builder().from(from_mailbox);

    for addr in &compose.to {
        let mb: Mailbox = if let Some(name) = &addr.name {
            format!("{} <{}>", name, addr.email)
                .parse()
                .map_err(|e| anyhow!("Invalid to address: {}", e))?
        } else {
            addr.email
                .parse()
                .map_err(|e| anyhow!("Invalid to address: {}", e))?
        };
        builder = builder.to(mb);
    }

    if let Some(cc_list) = &compose.cc {
        for addr in cc_list {
            let mb: Mailbox = addr.email.parse()?;
            builder = builder.cc(mb);
        }
    }

    builder = builder.subject(&compose.subject);

    if let Some(irt) = &compose.in_reply_to {
        builder = builder.in_reply_to(irt.parse()?);
    }

    let email = if let Some(html) = &compose.body_html {
        builder.multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(compose.body_text.clone()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html.clone()),
                ),
        )?
    } else {
        builder.body(compose.body_text.clone())?
    };

    let mailer = build_transport(account).await?;
    mailer.send(email).await?;
    info!("Message sent via SMTP for {}", account.email);
    Ok(())
}

async fn build_transport(account: &Account) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    match account.provider.as_str() {
        "gmail" | "outlook" => {
            // OAuth2 accounts must use XOAUTH2 SASL — a raw access token is not a password.
            let access_token = get_or_refresh_token(account).await?;
            let creds = Credentials::new(account.email.clone(), access_token);
            let transport =
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)?
                    .port(account.smtp_port)
                    .credentials(creds)
                    .authentication(vec![Mechanism::Xoauth2])
                    .build();
            Ok(transport)
        }
        _ => {
            // Plain-password accounts use LOGIN/PLAIN.
            let password = keychain::get_token(&account.id, "password")?
                .ok_or_else(|| anyhow!("No password stored for SMTP (account: {})", account.email))?;
            let creds = Credentials::new(account.email.clone(), password);
            let transport =
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)?
                    .port(account.smtp_port)
                    .credentials(creds)
                    .build();
            Ok(transport)
        }
    }
}

/// Mirrors imap.rs: refreshes via refresh_token if available, otherwise returns stored access_token.
async fn get_or_refresh_token(account: &Account) -> Result<String> {
    let refresh = keychain::get_token(&account.id, "refresh_token")?;
    if refresh.is_none() {
        return keychain::get_token(&account.id, "access_token")?
            .ok_or_else(|| anyhow!("No tokens for {}; re-auth required", account.email));
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
    let tokens = oauth::refresh_token(&config, &refresh).await?;
    keychain::store_token(&account.id, "access_token", &tokens.access_token)?;
    if let Some(rt) = &tokens.refresh_token {
        keychain::store_token(&account.id, "refresh_token", rt)?;
    }
    Ok(tokens.access_token)
}
