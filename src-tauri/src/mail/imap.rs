use crate::auth::{keychain, oauth};
use crate::db::{models::*, Database};
use crate::mail::{parser, threading};
use anyhow::{anyhow, Result};
use async_imap::imap_proto::types::{AttributeValue, Response, Status};
use async_imap::{types::Flag, Client};
use async_native_tls::TlsConnector;
use chrono::{Duration as ChronoDuration, Utc};
use futures::TryStreamExt;
use std::collections::{BTreeSet, HashMap};
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
const BATCH_SIZES: &[u32] = &[10, 25, 50, 100];
/// On first sync (no prior uid_next), only grab the most recent N messages.
const INITIAL_SYNC_LIMIT: u32 = 100;

pub async fn sync_mailbox(
    session: &mut ImapSession,
    account: &Account,
    mailbox: &mut Mailbox,
    db: Arc<Mutex<Database>>,
    on_progress: impl Fn(&str),
) -> Result<Vec<(Message, Vec<Attachment>)>> {
    let select = session.select(&mailbox.name).await?;

    let is_fresh = select.uid_validity != mailbox.uid_validity || mailbox.uid_next.is_none();
    let server_uid_next = select.uid_next.unwrap_or(select.exists + 1);

    mailbox.uid_validity = select.uid_validity;

    if select.exists == 0 {
        mailbox.uid_next = select.uid_next;
        return Ok(vec![]);
    }

    let mut all_results = Vec::new();
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
                all_results.len()
            ));

            let fetches: Vec<_> = session
                .uid_fetch(&uid_range, "(BODY.PEEK[] FLAGS UID)")
                .await?
                .try_collect()
                .await?;
            let gmail_labels = if account.provider == "gmail" {
                fetch_all_gmail_labels(session, &uid_range).await?
            } else {
                HashMap::new()
            };

            cursor = batch_start.saturating_sub(1);
            // Process this batch
            let batch_results = parse_fetches(&fetches, account, mailbox, &gmail_labels);
            if !batch_results.is_empty() {
                persist_batch(&batch_results, account, mailbox, &db).await?;
            }
            all_results.extend(batch_results);

            if cursor < 1 {
                break;
            }
        }
        mailbox.last_synced_at = Some(Utc::now());
    } else {
        // Incremental sync: fetch messages received SINCE the last sync time
        // This handles UID changes and avoids gaps if the server moves UIDs
        let sync_start_time = Utc::now();
        let last_sync = mailbox
            .last_synced_at
            .unwrap_or_else(|| Utc::now() - ChronoDuration::days(1));

        // IMAP SINCE only has day resolution, so we subtract one extra day to be safe
        let since_date = (last_sync - ChronoDuration::days(1))
            .format("%d-%b-%Y")
            .to_string();
        let search_query = format!("SINCE {}", since_date);

        on_progress("Checking for new mail…");
        let uids_set = session.uid_search(&search_query).await?;
        let mut uids: Vec<u32> = uids_set.into_iter().collect();
        uids.sort_unstable(); // For incremental, ascending is fine

        // Filter out UIDs we already have (best effort optimization)
        // In a more complex system we'd check the DB for existing UIDs here.

        if !uids.is_empty() {
            for chunk in uids.chunks(BATCH_SIZES[BATCH_SIZES.len() - 1] as usize) {
                let uid_range = format_uid_sequence_set(chunk);
                on_progress(&format!("Fetching new mail… {} so far", all_results.len()));

                let fetches: Vec<_> = session
                    .uid_fetch(&uid_range, "(BODY.PEEK[] FLAGS UID)")
                    .await?
                    .try_collect()
                    .await?;

                let gmail_labels = if account.provider == "gmail" {
                    fetch_all_gmail_labels(session, &uid_range).await?
                } else {
                    HashMap::new()
                };

                let batch_results = parse_fetches(&fetches, account, mailbox, &gmail_labels);
                if !batch_results.is_empty() {
                    persist_batch(&batch_results, account, mailbox, &db).await?;
                }
                all_results.extend(batch_results);
            }
        }

        mailbox.last_synced_at = Some(sync_start_time);
    }

    mailbox.uid_next = select.uid_next;
    mailbox.last_synced_at = Some(Utc::now());
    {
        let db = db.lock().await;
        db.upsert_mailbox(mailbox)?;
    }

    // Refresh metadata for existing messages that have attachments but no records in the attachments table
    refresh_missing_attachments(session, account, mailbox, &db, &on_progress).await?;

    info!(
        "Synced {} messages from {}/{}",
        all_results.len(),
        account.email,
        mailbox.name
    );

    Ok(all_results)
}

async fn refresh_missing_attachments(
    session: &mut ImapSession,
    account: &Account,
    mailbox: &Mailbox,
    db: &Arc<Mutex<Database>>,
    on_progress: &impl Fn(&str),
) -> Result<()> {
    let missing_uids = {
        let db = db.lock().await;
        db.get_messages_missing_attachments(&mailbox.id)?
    };

    if missing_uids.is_empty() {
        return Ok(());
    }

    on_progress(&format!(
        "Refreshing {} messages with attachments…",
        missing_uids.len()
    ));
    info!(
        "Refreshing {} messages with attachments for {}",
        missing_uids.len(),
        mailbox.name
    );

    for chunk in missing_uids.chunks(50) {
        let uid_range = format_uid_sequence_set(chunk);
        let fetches: Vec<_> = session
            .uid_fetch(&uid_range, "(BODY.PEEK[] FLAGS UID)")
            .await?
            .try_collect()
            .await?;

        let gmail_labels = if account.provider == "gmail" {
            fetch_all_gmail_labels(session, &uid_range).await?
        } else {
            HashMap::new()
        };

        let batch_results = parse_fetches(&fetches, account, mailbox, &gmail_labels);
        if !batch_results.is_empty() {
            persist_batch(&batch_results, account, mailbox, db).await?;
        }
    }

    Ok(())
}

pub fn format_uid_sequence_set(uids: &[u32]) -> String {
    if uids.is_empty() {
        return String::new();
    }

    let mut sorted = uids.to_vec();
    sorted.sort_unstable();

    let mut ranges = Vec::new();
    let mut start = sorted[0];
    let mut prev = sorted[0];
    for &uid in &sorted[1..] {
        if uid == prev + 1 {
            prev = uid;
            continue;
        }
        if start == prev {
            ranges.push(start.to_string());
        } else {
            ranges.push(format!("{}:{}", start, prev));
        }
        start = uid;
        prev = uid;
    }

    if start == prev {
        ranges.push(start.to_string());
    } else {
        ranges.push(format!("{}:{}", start, prev));
    }

    ranges.join(",")
}

pub fn parse_fetches(
    fetches: &[async_imap::types::Fetch],
    account: &Account,
    mailbox: &Mailbox,
    gmail_labels: &HashMap<u32, Vec<String>>,
) -> Vec<(Message, Vec<Attachment>)> {
    let mut results = Vec::new();
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
            Ok((mut msg, atts)) => {
                msg.flags = fetch
                    .flags()
                    .map(|f| flag_to_string(&f))
                    .filter(|s| !s.is_empty())
                    .collect();
                msg.is_read = msg.flags.iter().any(|f| f == "\\Seen");
                msg.is_flagged = msg.flags.iter().any(|f| f == "\\Flagged");
                if let Some(raw_labels) = gmail_labels.get(&uid) {
                    println!(">>> GMAIL LABELS uid={}: {:?}", uid, raw_labels);
                    // VibeMail/* labels → append to flags (existing categorisation system)
                    for label in raw_labels {
                        if let Some(vibe) = extract_vibemail_label(label) {
                            msg.flags.push(format!("VibeMail/{}", vibe));
                        }
                    }
                    // System labels → resolve to mailbox IDs for inbox_mailboxes
                    msg.inbox_mailboxes = raw_labels
                        .iter()
                        .filter_map(|label| {
                            let resolved = gmail_label_to_mailbox_name(label);
                            if resolved.is_none() && (label.starts_with('\\') || label.starts_with("\\\\")) {
                                println!(">>> GMAIL SYSTEM LABEL UNRESOLVED: {:?} (bytes: {:?})", label, label.as_bytes());
                            }
                            resolved.map(|mb_name| format!("{}:{}", account.id, mb_name))
                        })
                        .collect();
                    println!(">>> INBOX_MAILBOXES uid={}: {:?}", uid, msg.inbox_mailboxes);
                } else {
                    println!(">>> GMAIL LABELS uid={}: NO LABELS RETURNED", uid);
                }
                // Always include the mailbox we're syncing from.
                // For Gmail All Mail: ensures messages appear under All Mail in sidebar.
                // For non-Gmail: sets the single canonical mailbox.
                if !msg.inbox_mailboxes.contains(&mailbox.id) {
                    msg.inbox_mailboxes.push(mailbox.id.clone());
                }
                results.push((msg, atts));
            }
            Err(e) => warn!("Failed to parse uid={}: {}", uid, e),
        }
    }
    results
}

pub async fn persist_batch(
    batch: &[(Message, Vec<Attachment>)],
    account: &Account,
    mailbox: &Mailbox,
    db: &Arc<Mutex<Database>>,
) -> Result<()> {
    // 1. Collect all messages for threading: those in the batch + those already in the DB for affected threads
    let mut all_thread_messages = Vec::new();

    // Start with all messages in the current batch
    for (msg, _) in batch {
        all_thread_messages.push(msg.clone());
    }

    // 2-3. Find existing threads for these messages (by thread_id field OR by Message-ID header)
    // and pull their messages in so threading can merge correctly.
    // Matching by Message-ID header catches cross-mailbox duplicates (e.g. Gmail labels:
    // the same email synced from INBOX and [Gmail]/All Mail has the same header but different UIDs).
    {
        let message_ids: Vec<String> = batch
            .iter()
            .filter_map(|(msg, _)| msg.message_id.clone())
            .collect();

        let db_lock = db.lock().await;

        let mut thread_ids: BTreeSet<String> = BTreeSet::new();

        // Match by Message-ID header (cross-mailbox dedup)
        let mid_to_tid = db_lock.get_thread_ids_by_message_ids(&message_ids)?;
        for tid in mid_to_tid.into_values() {
            thread_ids.insert(tid);
        }

        // Also match by thread_id already set on batch messages (same-mailbox re-sync)
        for (msg, _) in batch {
            if let Some(tid) = &msg.thread_id {
                thread_ids.insert(tid.clone());
            }
        }

        for tid in thread_ids {
            let existing = db_lock.get_thread_messages(&tid)?;
            for ext_msg in existing {
                // Don't add if we already have this exact message row in our batch (batch is newer)
                if !all_thread_messages.iter().any(|m| m.id == ext_msg.id) {
                    all_thread_messages.push(ext_msg);
                }
            }
        }
    }

    // 4. Build updated thread objects
    let updated_threads = threading::build_threads(all_thread_messages, &account.id);

    // 5. Persist everything
    let db_lock = db.lock().await;
    db_lock.upsert_mailbox(mailbox)?;

    let mut thread_count = 0;
    let mut msg_count = 0;

    for thread in &updated_threads {
        if let Err(e) = db_lock.upsert_thread(thread) {
            println!(">>> DB ERROR: upsert_thread failed: {}", e);
            return Err(e);
        }
        thread_count += 1;
        if let Some(msgs) = &thread.messages {
            for msg in msgs {
                if let Err(e) = db_lock.upsert_message(msg) {
                    println!(">>> DB ERROR: upsert_message failed: {}", e);
                    return Err(e);
                }
                msg_count += 1;
                // Only update attachments for messages that were in our current batch
                if let Some((_, atts)) = batch.iter().find(|(m, _)| m.id == msg.id) {
                    db_lock.delete_message_attachments(&msg.id)?;
                    for att in atts {
                        if let Err(e) = db_lock.upsert_attachment(att) {
                            println!(">>> DB ERROR: upsert_attachment failed: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
        }
    }
    println!(
        ">>> DB: Successfully persisted {} threads and {} messages",
        thread_count, msg_count
    );
    info!(
        "Persisted {} threads and {} messages",
        thread_count, msg_count
    );

    // Refresh denormalized tables after batch insert
    db_lock.refresh_thread_mailboxes(&account.id)?;
    db_lock.refresh_mailbox_counts(&account.id)?;

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
            last_synced_at: None,
            thread_count: 0,
            unread_count: 0,
            folder_role: detect_folder_role(b.name()),
        })
        .collect();
    Ok(mailboxes)
}

/// Derive a folder_role from the mailbox name.
fn detect_folder_role(name: &str) -> Option<String> {
    let upper = name.to_uppercase();
    if upper == "INBOX" {
        Some("inbox".to_string())
    } else if upper.contains("SENT") {
        Some("sent".to_string())
    } else if upper.contains("DRAFT") {
        Some("drafts".to_string())
    } else if upper.contains("TRASH") {
        Some("trash".to_string())
    } else if upper.contains("SPAM") || upper.contains("JUNK") {
        Some("spam".to_string())
    } else if upper.contains("ALL MAIL") {
        Some("all_mail".to_string())
    } else if upper.contains("STAR") {
        Some("starred".to_string())
    } else if upper.contains("IMPORTANT") {
        Some("important".to_string())
    } else {
        None
    }
}

/// Fetch ALL X-GM-LABELS for the given UID range, returning raw label strings per UID.
/// This replaces the old VibeMail-only filter — callers decide which labels to keep.
pub async fn fetch_all_gmail_labels(
    session: &mut ImapSession,
    uid_range: &str,
) -> Result<HashMap<u32, Vec<String>>> {
    let mut labels_by_uid: HashMap<u32, Vec<String>> = HashMap::new();
    let cmd = format!("UID FETCH {} (UID X-GM-LABELS)", uid_range);
    let request_id = session.run_command(&cmd).await?;

    loop {
        let response = session
            .read_response()
            .await
            .ok_or_else(|| anyhow!("IMAP connection lost while fetching Gmail labels"))??;
        match response.parsed() {
            Response::Fetch(_, attrs) => {
                let mut uid: Option<u32> = None;
                let mut labels = Vec::new();
                for attr in attrs {
                    match attr {
                        AttributeValue::Uid(value) => uid = Some(*value),
                        AttributeValue::GmailLabels(values) => {
                            labels.extend(
                                values
                                    .iter()
                                    .map(|value| value.as_ref().to_string()),
                            );
                        }
                        _ => {}
                    }
                }
                if let Some(uid) = uid {
                    labels_by_uid.insert(uid, labels);
                }
            }
            Response::Done { tag, status, .. } if tag == &request_id => match status {
                Status::Ok => break,
                Status::No | Status::Bad => {
                    return Err(anyhow!("IMAP UID FETCH X-GM-LABELS failed"));
                }
                _ => break,
            },
            _ => {}
        }
    }

    Ok(labels_by_uid)
}

/// Map a Gmail system label (from X-GM-LABELS) to the local mailbox name.
/// Returns None for unknown or user-defined labels.
fn gmail_label_to_mailbox_name(label: &str) -> Option<&str> {
    let trimmed = label.trim_matches('"');
    // imap-proto returns system labels with double backslashes (e.g., "\\Inbox")
    // so we strip one leading backslash before matching.
    let normalized = trimmed.strip_prefix('\\').unwrap_or(trimmed);
    match normalized {
        "\\Inbox" => Some("INBOX"),
        "\\Sent" => Some("[Gmail]/Sent Mail"),
        "\\Draft" => Some("[Gmail]/Drafts"),
        "\\Spam" => Some("[Gmail]/Spam"),
        "\\Trash" => Some("[Gmail]/Trash"),
        "\\Important" => Some("[Gmail]/Important"),
        "\\Starred" => Some("[Gmail]/Starred"),
        _ => None,
    }
}

fn extract_vibemail_label(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches('"');
    let value = trimmed.strip_prefix("VibeMail/")?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}
