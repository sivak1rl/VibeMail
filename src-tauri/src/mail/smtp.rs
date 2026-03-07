use crate::auth::keychain;
use crate::db::models::{Account, ComposeMessage};
use anyhow::{anyhow, Result};
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
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
    let password = keychain::get_token(&account.id, "access_token")
        .ok()
        .flatten()
        .or_else(|| keychain::get_token(&account.id, "password").ok().flatten())
        .ok_or_else(|| {
            anyhow!(
                "No credentials stored for SMTP (account: {})",
                account.email
            )
        })?;

    let creds = Credentials::new(account.email.clone(), password);
    let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)?
        .port(account.smtp_port)
        .credentials(creds)
        .build();
    Ok(transport)
}
