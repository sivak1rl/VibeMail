use crate::auth::{keychain, oauth};
use crate::db::{models::*, Database};
use crate::mail::{parser, threading};
use anyhow::{anyhow, Result};
use async_imap::{types::Flag, Client};
use async_native_tls::TlsConnector;
use futures::TryStreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tracing::{debug, info, warn};

pub type ImapSession =
    async_imap::Session<async_native_tls::TlsStream<tokio_util::compat::Compat<TcpStream>>>;

/// XOAUTH2 authenticator for async-imap.
/// First call sends the encoded token; subsequent calls (error challenges from server)
/// send an empty response to cleanly terminate the SASL exchange.
struct XOAuth2 {
    encoded: String,
    sent: bool,
}

impl async_imap::Authenticator for XOAuth2 {
    type Response = String;
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        if !self.sent {
            self.sent = true;
            self.encoded.clone()
        } else {
            String::new()
        }
    }
}

pub async fn connect_imap(account: &Account) -> Result<ImapSession> {
    let tls = TlsConnector::new();

    // Resolve and prefer IPv4 — some environments have broken IPv6 that hangs
    let addr = {
        use tokio::net::lookup_host;
        let addrs: Vec<_> = lookup_host(format!("{}:{}", account.imap_host, account.imap_port))
            .await?
            .collect();
        debug!(
            "Resolved {} to {} addresses",
            account.imap_host,
            addrs.len()
        );
        addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or(addrs.first())
            .copied()
            .ok_or_else(|| anyhow!("DNS resolved no addresses for {}", account.imap_host))?
    };

    let tcp = tokio::time::timeout(Duration::from_secs(15), TcpStream::connect(addr))
        .await
        .map_err(|_| anyhow!("IMAP TCP connect timed out"))??;

    let tls_stream = tokio::time::timeout(
        Duration::from_secs(15),
        tls.connect(&account.imap_host, tcp.compat()),
    )
    .await
    .map_err(|_| anyhow!("IMAP TLS handshake timed out"))??;

    let mut client = Client::new(tls_stream);

    // Consume the server greeting (e.g. "* OK Gimap ready") before sending commands.
    // Without this, authenticate()/login() reads the greeting instead of the command
    // response and hangs forever in async-imap 0.10.
    let _greeting = client.read_response().await;

    let session = match account.provider.as_str() {
        "gmail" | "outlook" => {
            let access_token = get_or_refresh_token(account).await?;
            let encoded = oauth::build_xoauth2(&account.email, &access_token);
            tokio::time::timeout(
                Duration::from_secs(15),
                client.authenticate(
                    "XOAUTH2",
                    XOAuth2 {
                        encoded,
                        sent: false,
                    },
                ),
            )
            .await
            .map_err(|_| anyhow!("IMAP XOAUTH2 auth timed out"))?
            .map_err(|(e, _)| anyhow!("IMAP XOAUTH2 auth failed: {}", e))?
        }
        _ => {
            let password = keychain::get_token(&account.id, "password")?
                .ok_or_else(|| anyhow!("No password for account {}", account.email))?;
            client
                .login(&account.email, &password)
                .await
                .map_err(|(e, _)| anyhow!("IMAP login failed: {}", e))?
        }
    };

    info!("IMAP connected for {}", account.email);
    Ok(session)
}

async fn get_or_refresh_token(account: &Account) -> Result<String> {
    // Always refresh if we have a refresh token — access tokens expire after ~1 hour
    let refresh = keychain::get_token(&account.id, "refresh_token")?;
    if refresh.is_none() {
        // No refresh token — try the stored access token as-is
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

    let tokens = oauth::refresh_token(&config, &refresh).await?;
    keychain::store_token(&account.id, "access_token", &tokens.access_token)?;
    if let Some(rt) = &tokens.refresh_token {
        keychain::store_token(&account.id, "refresh_token", rt)?;
    }
    Ok(tokens.access_token)
}

fn flag_to_string(f: &Flag<'_>) -> String {
    match f {
        Flag::Seen => "\\Seen".to_string(),
        Flag::Answered => "\\Answered".to_string(),
        Flag::Flagged => "\\Flagged".to_string(),
        Flag::Deleted => "\\Deleted".to_string(),
        Flag::Draft => "\\Draft".to_string(),
        Flag::Recent => "\\Recent".to_string(),
        Flag::Custom(s) => s.to_string(),
        _ => String::new(),
    }
}

/// Graduated batch sizes: start tiny for instant feedback, ramp up for throughput.
const BATCH_SIZES: &[u32] = &[10, 25, 50, 100, 200, 500];
/// On first sync (no prior uid_next), only grab the most recent N messages.
const INITIAL_SYNC_LIMIT: u32 = 500;

pub async fn sync_mailbox(
    session: &mut ImapSession,
    account: &Account,
    mailbox: &mut Mailbox,
    db: Arc<Mutex<Database>>,
    on_progress: impl Fn(&str),
) -> Result<Vec<Message>> {
    let select = session.select(&mailbox.name).await?;

    let is_fresh = select.uid_validity != mailbox.uid_validity || mailbox.uid_next.is_none();
    let server_uid_next = select.uid_next.unwrap_or(select.exists + 1);

    mailbox.uid_validity = select.uid_validity;

    if select.exists == 0 {
        mailbox.uid_next = select.uid_next;
        return Ok(vec![]);
    }

    let mut all_messages = Vec::new();
    let mut batch_idx: usize = 0;

    if is_fresh {
        // First sync: fetch newest-first (descending) in graduated batches
        let mut cursor = server_uid_next.saturating_sub(1); // highest possible UID
        let stop_at = server_uid_next.saturating_sub(INITIAL_SYNC_LIMIT).max(1);

        while cursor >= stop_at && cursor >= 1 {
            let batch_size = BATCH_SIZES[batch_idx.min(BATCH_SIZES.len() - 1)];
            batch_idx += 1;
            let batch_start = cursor.saturating_sub(batch_size - 1).max(stop_at);
            let uid_range = format!("{}:{}", batch_start, cursor);

            on_progress(&format!(
                "Fetching newest mail… {} so far",
                all_messages.len()
            ));

            info!(
                "Fetching UIDs {}..{} from {}/{}",
                batch_start, cursor, account.email, mailbox.name
            );

            let fetches: Vec<_> = session
                .uid_fetch(&uid_range, "(RFC822 FLAGS UID)")
                .await?
                .try_collect()
                .await?;

            cursor = batch_start.saturating_sub(1);
            // Process this batch (continues below via shared parsing code)
            let batch_messages = parse_fetches(&fetches, account, mailbox);
            if !batch_messages.is_empty() {
                persist_batch(&batch_messages, account, mailbox, &db).await?;
            }
            all_messages.extend(batch_messages);

            if cursor < 1 {
                break;
            }
        }
    } else {
        // Incremental sync: fetch from last uid_next upward
        let fetch_from = mailbox.uid_next.unwrap_or(1);
        let mut batch_start = fetch_from;

        while batch_start < server_uid_next {
            let batch_size = BATCH_SIZES[batch_idx.min(BATCH_SIZES.len() - 1)];
            batch_idx += 1;
            let batch_end = (batch_start + batch_size - 1).min(server_uid_next);
            let uid_range = format!("{}:{}", batch_start, batch_end);

            on_progress(&format!("Fetching new mail… {} so far", all_messages.len()));

            info!(
                "Fetching UIDs {}..{} from {}/{}",
                batch_start, batch_end, account.email, mailbox.name
            );

            let fetches: Vec<_> = session
                .uid_fetch(&uid_range, "(RFC822 FLAGS UID)")
                .await?
                .try_collect()
                .await?;

            let batch_messages = parse_fetches(&fetches, account, mailbox);
            if !batch_messages.is_empty() {
                persist_batch(&batch_messages, account, mailbox, &db).await?;
            }
            all_messages.extend(batch_messages);
            batch_start = batch_end + 1;
        }
    }

    mailbox.uid_next = select.uid_next;
    {
        let db = db.lock().await;
        db.upsert_mailbox(mailbox)?;
    }

    info!(
        "Synced {} messages from {}/{}",
        all_messages.len(),
        account.email,
        mailbox.name
    );

    Ok(all_messages)
}

fn parse_fetches(
    fetches: &[async_imap::types::Fetch],
    account: &Account,
    mailbox: &Mailbox,
) -> Vec<Message> {
    let mut messages = Vec::new();
    for fetch in fetches {
        let uid = match fetch.uid {
            Some(u) => u,
            None => continue,
        };
        let raw = match fetch.body() {
            Some(b) => b,
            None => continue,
        };
        let msg_id = format!("{}:{}:{}", account.id, mailbox.id, uid);
        match parser::parse_message(raw, &msg_id, &account.id, &mailbox.id, uid) {
            Ok(mut msg) => {
                msg.flags = fetch
                    .flags()
                    .map(|f| flag_to_string(&f))
                    .filter(|s| !s.is_empty())
                    .collect();
                messages.push(msg);
            }
            Err(e) => warn!("Failed to parse uid={}: {}", uid, e),
        }
    }
    messages
}

async fn persist_batch(
    messages: &[Message],
    account: &Account,
    mailbox: &Mailbox,
    db: &Arc<Mutex<Database>>,
) -> Result<()> {
    let threads = threading::build_threads(messages.to_vec(), &account.id);
    let db = db.lock().await;
    db.upsert_mailbox(mailbox)?;
    for thread in &threads {
        db.upsert_thread(thread)?;
        if let Some(msgs) = &thread.messages {
            for msg in msgs {
                db.upsert_message(msg)?;
            }
        }
    }
    Ok(())
}

pub async fn list_mailboxes(session: &mut ImapSession, account_id: &str) -> Result<Vec<Mailbox>> {
    let boxes: Vec<_> = session
        .list(Some(""), Some("*"))
        .await?
        .try_collect()
        .await?;
    let mailboxes = boxes
        .iter()
        .map(|b| Mailbox {
            id: format!("{}:{}", account_id, b.name()),
            account_id: account_id.to_string(),
            name: b.name().to_string(),
            delimiter: b.delimiter().map(|d| d.to_string()),
            flags: b.attributes().iter().map(|a| format!("{:?}", a)).collect(),
            uid_validity: None,
            uid_next: None,
        })
        .collect();
    Ok(mailboxes)
}
