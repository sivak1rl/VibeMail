use crate::auth::{keychain, token_cache};
use crate::db::models::{Account, ComposeMessage};
use crate::mail::retry;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::{Credentials, Mechanism},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use tracing::info;

pub async fn send_message(account: &Account, compose: &ComposeMessage) -> Result<()> {
    // Build the email message once (this is pure validation, not network I/O)
    let email = build_message(account, compose)?;

    // Send with retry — transient SMTP failures get retried automatically
    let account = account.clone();
    retry::with_retry("SMTP send", || {
        let account = account.clone();
        let email = email.clone();
        async move {
            let mailer = build_transport(&account).await?;
            mailer.send(email).await?;
            info!("Message sent via SMTP for {}", account.email);
            Ok(())
        }
    })
    .await
}

fn build_message(account: &Account, compose: &ComposeMessage) -> Result<Message> {
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

    let body_part: MultiPart = if let Some(html) = &compose.body_html {
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
            )
    } else {
        MultiPart::alternative().singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(compose.body_text.clone()),
        )
    };

    let attachments = compose.attachments.as_deref().unwrap_or(&[]);
    if attachments.is_empty() {
        Ok(builder.multipart(body_part)?)
    } else {
        let mut mixed = MultiPart::mixed().multipart(body_part);
        for attach in attachments {
            let data = STANDARD
                .decode(&attach.data_base64)
                .map_err(|e| anyhow!("base64 decode error: {}", e))?;
            let ct: ContentType = attach
                .content_type
                .parse()
                .unwrap_or_else(|_| "application/octet-stream".parse().unwrap());
            let part = Attachment::new(attach.filename.clone()).body(data, ct);
            mixed = mixed.singlepart(part);
        }
        Ok(builder.multipart(mixed)?)
    }
}

async fn build_transport(account: &Account) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    match account.provider.as_str() {
        "gmail" | "outlook" => {
            // OAuth2 accounts must use XOAUTH2 SASL — a raw access token is not a password.
            let access_token = token_cache::get_access_token(account).await?;
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
            let password = keychain::get_token(&account.id, "password")?.ok_or_else(|| {
                anyhow!("No password stored for SMTP (account: {})", account.email)
            })?;
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
