use crate::db::{models::*, Database};
use crate::mail::{imap as mail_imap, sync::SyncManager};
use crate::mail::imap::format_uid_sequence_set;
use crate::search::SearchIndex;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub account_id: String,
    pub mailbox_id: Option<String>,
    pub new_messages: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncAccountRequest {
    pub account_id: String,
    pub mailbox_id: Option<String>,
    pub days: Option<u32>,
    pub limit: Option<u32>,
}

#[tauri::command]
pub async fn fetch_history(
    request: SyncAccountRequest,
    app: AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    search: State<'_, Arc<Mutex<SearchIndex>>>,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
) -> Result<SyncResult, String> {
    let account_id = request.account_id.clone();
    let mailbox_id = request.mailbox_id.clone();
    let days = request.days.unwrap_or(30);
    let limit = request.limit.unwrap_or(100);

    {
        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr.lock().await;
        let key = if let Some(mid) = &mailbox_id { format!("history:{}:{}", account_id, mid) } else { format!("history:{}", account_id) };
        if mgr.is_syncing(&key) {
            return Ok(SyncResult {
                account_id,
                mailbox_id,
                new_messages: 0,
                error: Some("History fetch already in progress".to_string()),
            });
        }
        mgr.start_sync(&key);
    }

    let db_clone = db.inner().clone();
    let search_clone = search.inner().clone();
    let sync_mgr_clone = sync_mgr.inner().clone();
    let app_clone = app.clone();
    let account_id_task = account_id.clone();
    let mailbox_id_task = mailbox_id.clone();

    tokio::spawn(async move {
        let result = async {
            let account = {
                let db = db_clone.lock().await;
                db.list_accounts()?
                    .into_iter()
                    .find(|a| a.id == account_id_task)
                    .ok_or_else(|| anyhow::anyhow!("Account not found"))?
            };

            let mut session = mail_imap::connect_imap(&account).await?;
            let mut total_new = 0;

            let mailboxes = if let Some(mid) = &mailbox_id_task {
                let db = db_clone.lock().await;
                vec![db.get_mailbox_by_id(&account.id, mid)?.ok_or_else(|| anyhow::anyhow!("Mailbox not found"))?]
            } else {
                let db = db_clone.lock().await;
                db.list_mailboxes(&account.id)?
            };

            for mailbox in mailboxes {
                let oldest_date: Option<DateTime<Utc>> = {
                    let db = db_clone.lock().await;
                    db.get_mailbox_oldest_date(&mailbox.id)?
                };

                println!(">>> HISTORY: Mailbox {} has oldest local date: {:?}", mailbox.name, oldest_date);

                let search_query = if let Some(oldest) = oldest_date {
                    let before_date = oldest.format("%d-%b-%Y").to_string();
                    format!("BEFORE {}", before_date)
                } else {
                    let since_date = (Utc::now() - ChronoDuration::days(days as i64)).format("%d-%b-%Y").to_string();
                    format!("SINCE {}", since_date)
                };

                println!(">>> HISTORY: Search query: {}", search_query);
                let _ = app_clone.emit("sync-progress", SyncProgress {
                    message: format!("Searching for history: {}…", mailbox.name),
                    current: None,
                    total: None,
                });

                let _select = session.select(&mailbox.name).await?;
                let uids_set = session.uid_search(&search_query).await?;
                let mut uids: Vec<u32> = uids_set.into_iter().collect();
                uids.sort_unstable_by(|a, b| b.cmp(a)); // Newest of the older mail first

                println!(">>> HISTORY: Found {} candidate UIDs", uids.len());

                // Limit the number of history items per pull
                uids.truncate(limit as usize);

                if !uids.is_empty() {
                    let batch_size = limit.min(500); // Caps individual IMAP fetch to 500 for safety
                    let total = uids.len();
                    for (idx, chunk) in uids.chunks(batch_size as usize).enumerate() {
                        let uid_range = format_uid_sequence_set(chunk);
                        println!(">>> HISTORY: Fetching UID range: {}", uid_range);
                        let current_count = (idx * batch_size as usize) + chunk.len();
                        let _ = app_clone.emit("sync-progress", SyncProgress {
                            message: format!("Fetching {} history items for {}…", total, mailbox.name),
                            current: Some(current_count),
                            total: Some(total),
                        });


                        let fetches: Vec<_> = session
                            .uid_fetch(&uid_range, "(BODY.PEEK[] FLAGS UID)")
                            .await?
                            .try_collect()
                            .await?;

                        println!(">>> HISTORY: Parsed {} fetches from server", fetches.len());

                        let gmail_labels = if account.provider == "gmail" {
                            crate::mail::imap::fetch_gmail_vibemail_labels(&mut session, &uid_range).await?
                        } else {
                            HashMap::new()
                        };

                        let batch_results = crate::mail::imap::parse_fetches(&fetches, &account, &mailbox, &gmail_labels);
                        println!(">>> HISTORY: Parsed {} messages from fetches", batch_results.len());
                        if !batch_results.is_empty() {
                            crate::mail::imap::persist_batch(&batch_results, &account, &mailbox, &db_clone).await?;
                            total_new += batch_results.len();
                            println!(">>> HISTORY: Persisted {} messages", batch_results.len());
                        }
                    }
                }
            }

            let _ = session.logout().await;
            Ok::<usize, anyhow::Error>(total_new)
        }.await;

        let err = match result {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr_clone.lock().await;
        let key = if let Some(mid) = &mailbox_id_task { format!("history:{}:{}", account_id_task, mid) } else { format!("history:{}", account_id_task) };
        mgr.finish_sync(&key, err);
    });

    Ok(SyncResult {
        account_id,
        mailbox_id,
        new_messages: 0,
        error: None,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListThreadsRequest {
    pub account_id: String,
    pub mailbox_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub focus_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListMailboxesRequest {
    pub account_id: String,
    pub refresh: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetThreadsReadRequest {
    pub thread_ids: Vec<String>,
    pub read: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetThreadsFlaggedRequest {
    pub thread_ids: Vec<String>,
    pub flagged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveThreadsRequest {
    pub thread_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxSummary {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub delimiter: Option<String>,
    pub flags: Vec<String>,
    pub uid_validity: Option<u32>,
    pub uid_next: Option<u32>,
    pub thread_count: u32,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub message: String,
    pub current: Option<usize>,
    pub total: Option<usize>,
}


impl From<crate::db::queries::MailboxStats> for MailboxSummary {
    fn from(value: crate::db::queries::MailboxStats) -> Self {
        Self {
            id: value.mailbox.id,
            account_id: value.mailbox.account_id,
            name: value.mailbox.name,
            delimiter: value.mailbox.delimiter,
            flags: value.mailbox.flags,
            uid_validity: value.mailbox.uid_validity,
            uid_next: value.mailbox.uid_next,
            thread_count: value.thread_count,
            unread_count: value.unread_count,
        }
    }
}

#[tauri::command]
pub async fn sync_account(
    request: SyncAccountRequest,
    app: AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    search: State<'_, Arc<Mutex<SearchIndex>>>,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
) -> Result<SyncResult, String> {
    let account_id = request.account_id.clone();
    let mailbox_id = request.mailbox_id.clone();

    {
        let mut mgr = sync_mgr.lock().await;
        if mgr.is_syncing(&account_id) {
            return Ok(SyncResult {
                account_id,
                mailbox_id,
                new_messages: 0,
                error: Some("Sync already in progress".to_string()),
            });
        }
        mgr.start_sync(&account_id);
    }

    // Clone for background task
    let db_clone = db.inner().clone();
    let search_clone = search.inner().clone();
    let sync_mgr_clone = sync_mgr.inner().clone();
    let app_clone = app.clone();
    let account_id_task = account_id.clone();
    let mailbox_id_task = mailbox_id.clone();

    tokio::spawn(async move {
        let result = if let Some(mid) = &mailbox_id_task {
            do_sync(
                &account_id_task,
                Some(mid),
                db_clone,
                search_clone,
                app_clone,
            )
            .await
        } else {
            sync_all_folders(&account_id_task, db_clone, search_clone, app_clone).await
        };

        let err = match result {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        let mut mgr = sync_mgr_clone.lock().await;
        mgr.finish_sync(&account_id_task, err);
    });

    // Return immediately to frontend
    Ok(SyncResult {
        account_id,
        mailbox_id,
        new_messages: 0,
        error: None,
    })
}

async fn sync_all_folders(
    account_id: &str,
    db: Arc<Mutex<Database>>,
    search: Arc<Mutex<SearchIndex>>,
    app: AppHandle,
) -> anyhow::Result<usize> {
    let mailboxes = {
        let db = db.lock().await;
        db.list_mailboxes(account_id)?
    };

    let total = mailboxes.len();
    let mut total_new = 0;
    for (i, mailbox) in mailboxes.into_iter().enumerate() {
        let _ = app.emit("sync-progress", SyncProgress {
            message: format!("Syncing {}…", mailbox.name),
            current: Some(i + 1),
            total: Some(total),
        });
        match do_sync(
            account_id,
            Some(&mailbox.id),
            db.clone(),
            search.clone(),
            app.clone(),
        )
        .await
        {
            Ok(n) => total_new += n,
            Err(e) => tracing::warn!("Failed to sync mailbox {}: {}", mailbox.name, e),
        }
    }

    Ok(total_new)
}

async fn do_sync(
    account_id: &str,
    mailbox_id: Option<&str>,
    db: Arc<Mutex<Database>>,
    search: Arc<Mutex<SearchIndex>>,
    app: AppHandle,
) -> anyhow::Result<usize> {
    use tokio::time::{timeout, Duration};

    let account = {
        let db = db.lock().await;
        db.list_accounts()?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?
    };

    let mut session = match timeout(Duration::from_secs(10), mail_imap::connect_imap(&account)).await {
        Ok(s) => s?,
        Err(_) => return Err(anyhow::anyhow!("Connection timeout")),
    };

    let mut mailbox = {
        let db = db.lock().await;
        if let Some(mailbox_id) = mailbox_id {
            db.get_mailbox_by_id(account_id, mailbox_id)?
                .ok_or_else(|| anyhow::anyhow!("Mailbox not found"))?
        } else {
            db.get_mailbox_by_name(account_id, "INBOX")?
                .unwrap_or_else(|| Mailbox {
                    id: format!("{}:INBOX", account_id),
                    account_id: account_id.to_string(),
                    name: "INBOX".to_string(),
                    delimiter: None,
                    flags: Vec::new(),
                    uid_validity: None,
                    uid_next: None,
                    last_synced_at: None,
                })
        }
    };

    let _ = app.emit("sync-progress", SyncProgress {
        message: format!("Syncing {}…", mailbox.name),
        current: None,
        total: None,
    });
    println!(">>> SYNC: Starting sync for {}", mailbox.name);

    // If it's a first-time sync (no uid_next), don't use a timeout.
    // Otherwise, use 60s to prevent background hangs.
    let is_fresh = mailbox.uid_next.is_none();

    let sync_future = mail_imap::sync_mailbox(&mut session, &account, &mut mailbox, db.clone(), {
        let app = app.clone();
        move |status: &str| {
            println!(">>> SYNC PROGRESS: {}", status);
            let _ = app.emit("sync-progress", SyncProgress {
                message: status.to_string(),
                current: None,
                total: None,
            });
        }
    });

    let messages = if is_fresh {
        match sync_future.await {
            Ok(msgs) => msgs,
            Err(e) => {
                println!(">>> SYNC ERROR: {}", e);
                return Err(e.into());
            }
        }
    } else {
        match timeout(Duration::from_secs(60), sync_future).await {
            Ok(res) => match res {
                Ok(msgs) => msgs,
                Err(e) => {
                    println!(">>> SYNC ERROR: {}", e);
                    return Err(e.into());
                }
            },
            Err(_) => {
                println!(">>> SYNC TIMEOUT for {}", mailbox.name);
                tracing::warn!("Sync timeout for mailbox {}", mailbox.name);
                return Ok(0);
            }
        }
    };

    let count = messages.len();
    println!(">>> SYNC: Downloaded {} messages for {}", count, mailbox.name);
    if count > 0 {
        let _ = app.emit("sync-progress", SyncProgress {
            message: format!("Indexing {} messages…", count),
            current: None,
            total: None,
        });

        let search = search.lock().await;
        for (msg, _) in &messages {
            if let Some(thread_id) = &msg.thread_id {
                let subject = msg.subject.as_deref().unwrap_or_default();
                let body = msg.body_text.as_deref().unwrap_or_default();
                let sender = msg
                    .from
                    .first()
                    .map(|a| a.email.as_str())
                    .unwrap_or_default();
                let _ = search.add_document(thread_id, subject, body, sender);
            }
        }
    }

    let _ = session.logout().await;
    Ok(count)
}

#[tauri::command]
pub async fn list_threads(
    request: ListThreadsRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Thread>, String> {
    let limit = request.limit.unwrap_or(50);
    let offset = request.offset.unwrap_or(0);
    let db = db.lock().await;
    let mut threads = db
        .list_threads(
            &request.account_id,
            request.mailbox_id.as_deref(),
            limit,
            offset,
        )
        .map_err(|e| e.to_string())?;

    if request.focus_only.unwrap_or(false) {
        threads.retain(|t| t.triage_score.unwrap_or(0.5) >= 0.6);
    }
    Ok(threads)
}

#[tauri::command]
pub async fn get_sync_status(
    account_id: String,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
) -> Result<bool, String> {
    let mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr.lock().await;
    // Check main sync
    if mgr.is_syncing(&account_id) { return Ok(true); }
    // Check history syncs
    for key in mgr.accounts.keys() {
        let k: &String = key;
        if k.contains(&account_id) && mgr.is_syncing(k) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn list_mailboxes(
    request: ListMailboxesRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<MailboxSummary>, String> {
    let account = {
        let db = db.lock().await;
        db.list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|a| a.id == request.account_id)
            .ok_or_else(|| "Account not found".to_string())?
    };

    if request.refresh.unwrap_or(false) {
        let mut session = mail_imap::connect_imap(&account)
            .await
            .map_err(|e| e.to_string())?;
        let mailboxes = mail_imap::list_mailboxes(&mut session, &request.account_id)
            .await
            .map_err(|e| e.to_string())?;

        {
            let db = db.lock().await;
            for mailbox in &mailboxes {
                db.upsert_mailbox(mailbox).map_err(|e| e.to_string())?;
            }
        }

        let _ = session.logout().await;
        let db = db.lock().await;
        let summaries = db
            .list_mailboxes_with_counts(&request.account_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(MailboxSummary::from)
            .collect();
        return Ok(summaries);
    }

    let cached = {
        let db = db.lock().await;
        db.list_mailboxes_with_counts(&request.account_id)
            .map_err(|e| e.to_string())?
    };

    if !cached.is_empty() {
        return Ok(cached.into_iter().map(MailboxSummary::from).collect());
    }

    let mut session = mail_imap::connect_imap(&account)
        .await
        .map_err(|e| e.to_string())?;
    let mailboxes = mail_imap::list_mailboxes(&mut session, &request.account_id)
        .await
        .map_err(|e| e.to_string())?;

    {
        let db = db.lock().await;
        for mailbox in &mailboxes {
            db.upsert_mailbox(mailbox).map_err(|e| e.to_string())?;
        }
    }

    let _ = session.logout().await;
    let db = db.lock().await;
    Ok(db
        .list_mailboxes_with_counts(&request.account_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(MailboxSummary::from)
        .collect())
}

#[tauri::command]
pub async fn get_thread(
    thread_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Message>, String> {
    let db = db.lock().await;
    db.get_thread_messages(&thread_id)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn mark_read(message_id: String) -> Result<(), String> {
    tracing::info!("mark_read: {}", message_id);
    Ok(())
}

#[tauri::command]
pub async fn set_threads_flagged(
    request: SetThreadsFlaggedRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<usize, String> {
    let thread_ids = request
        .thread_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if thread_ids.is_empty() {
        return Ok(0);
    }

    let (account, mailbox_targets) = {
        let db = db.lock().await;
        let locations = db
            .get_thread_message_locations(&thread_ids)
            .map_err(|e| e.to_string())?;
        if locations.is_empty() {
            return Ok(0);
        }

        let account_ids = locations
            .iter()
            .map(|location| location.account_id.clone())
            .collect::<BTreeSet<_>>();
        if account_ids.len() != 1 {
            return Err("Selected threads span multiple accounts".to_string());
        }
        let account_id = account_ids
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| "Missing account id".to_string())?;

        let account = db
            .list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|acct| acct.id == account_id)
            .ok_or_else(|| "Account not found".to_string())?;

        let mut uids_by_mailbox: HashMap<String, BTreeSet<u32>> = HashMap::new();
        for location in locations {
            uids_by_mailbox
                .entry(location.mailbox_id)
                .or_default()
                .insert(location.uid);
        }

        let mut targets = Vec::with_capacity(uids_by_mailbox.len());
        for (mailbox_id, uids) in uids_by_mailbox {
            let mailbox = db
                .get_mailbox_by_id(&account.id, &mailbox_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Mailbox not found: {}", mailbox_id))?;
            targets.push((mailbox.name, uids.into_iter().collect::<Vec<_>>()));
        }

        (account, targets)
    };

    let mut session = mail_imap::connect_imap(&account)
        .await
        .map_err(|e| e.to_string())?;
    let store_cmd = if request.flagged {
        "+FLAGS.SILENT (\\Flagged)"
    } else {
        "-FLAGS.SILENT (\\Flagged)"
    };

    for (mailbox_name, uids) in mailbox_targets {
        session
            .select(&mailbox_name)
            .await
            .map_err(|e| e.to_string())?;
        for chunk in uids.chunks(250) {
            let sequence_set = format_uid_sequence_set(chunk);
            let mut updates = session
                .uid_store(&sequence_set, store_cmd)
                .await
                .map_err(|e| e.to_string())?;
            while updates
                .try_next()
                .await
                .map_err(|e| e.to_string())?
                .is_some()
            {}
        }
    }

    let _ = session.logout().await;

    let db = db.lock().await;
    db.set_threads_flagged_state(&thread_ids, request.flagged)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn archive_threads(
    request: ArchiveThreadsRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<usize, String> {
    let thread_ids = request
        .thread_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if thread_ids.is_empty() {
        return Ok(0);
    }

    let (account, targets, archive_mailbox) = {
        let db = db.lock().await;
        let locations = db
            .get_thread_message_locations(&thread_ids)
            .map_err(|e| e.to_string())?;
        if locations.is_empty() {
            return Ok(0);
        }

        let account_ids = locations
            .iter()
            .map(|location| location.account_id.clone())
            .collect::<BTreeSet<_>>();
        if account_ids.len() != 1 {
            return Err("Selected threads span multiple accounts".to_string());
        }
        let account_id = account_ids
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| "Missing account id".to_string())?;

        let account = db
            .list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|acct| acct.id == account_id)
            .ok_or_else(|| "Account not found".to_string())?;

        let archive_mailbox = db
            .list_mailboxes(&account.id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|mb| {
                let n = mb.name.to_lowercase();
                // Check common names and IMAP attributes (stored in flags JSON)
                let is_archive_attr = mb.flags.iter().any(|f| {
                    let f = f.to_lowercase();
                    f.contains("archive") || f.contains("all")
                });
                n == "archive" || n == "all mail" || is_archive_attr
            })
            .ok_or_else(|| "No archive mailbox found. Ensure you have an 'Archive' or 'All Mail' folder.".to_string())?;

        let mut uids_by_mailbox: HashMap<String, BTreeSet<u32>> = HashMap::new();
        for location in locations {
            // Only move if not already in archive
            if location.mailbox_id != archive_mailbox.id {
                uids_by_mailbox
                    .entry(location.mailbox_id)
                    .or_default()
                    .insert(location.uid);
            }
        }

        if uids_by_mailbox.is_empty() {
            return Ok(0);
        }

        let mut targets = Vec::with_capacity(uids_by_mailbox.len());
        for (mailbox_id, uids) in uids_by_mailbox {
            let mailbox = db
                .get_mailbox_by_id(&account.id, &mailbox_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Mailbox not found: {}", mailbox_id))?;
            targets.push((mailbox.name, uids.into_iter().collect::<Vec<_>>()));
        }

        (account, targets, archive_mailbox)
    };

    let archive_mailbox_name = &archive_mailbox.name;
    let mut session = mail_imap::connect_imap(&account)
        .await
        .map_err(|e| e.to_string())?;

    for (mailbox_name, uids) in targets {
        session
            .select(&mailbox_name)
            .await
            .map_err(|e| e.to_string())?;
        for chunk in uids.chunks(250) {
            let sequence_set = format_uid_sequence_set(chunk);
            // Copy to archive
            session
                .uid_copy(&sequence_set, archive_mailbox_name)
                .await
                .map_err(|e| e.to_string())?;
            // Mark for deletion in original mailbox
            let mut updates = session
                .uid_store(&sequence_set, "+FLAGS.SILENT (\\Deleted)")
                .await
                .map_err(|e| e.to_string())?;
            while updates
                .try_next()
                .await
                .map_err(|e| e.to_string())?
                .is_some()
            {}
        }
        // Actually remove the messages marked \Deleted
        session.expunge().await.map_err(|e| e.to_string())?;
    }

    let _ = session.logout().await;

    // For simplicity, we trigger a full sync after archive to let the app reconcile state
    Ok(thread_ids.len())
}

#[tauri::command]
pub async fn set_threads_read(
    request: SetThreadsReadRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<usize, String> {
    let thread_ids = request
        .thread_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if thread_ids.is_empty() {
        return Ok(0);
    }

    let (account, mailbox_targets) = {
        let db = db.lock().await;
        let locations = db
            .get_thread_message_locations(&thread_ids)
            .map_err(|e| e.to_string())?;
        if locations.is_empty() {
            return Ok(0);
        }

        let account_ids = locations
            .iter()
            .map(|location| location.account_id.clone())
            .collect::<BTreeSet<_>>();
        if account_ids.len() != 1 {
            return Err("Selected threads span multiple accounts".to_string());
        }
        let account_id = account_ids
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| "Missing account id".to_string())?;

        let account = db
            .list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|acct| acct.id == account_id)
            .ok_or_else(|| "Account not found".to_string())?;

        let mut uids_by_mailbox: HashMap<String, BTreeSet<u32>> = HashMap::new();
        for location in locations {
            uids_by_mailbox
                .entry(location.mailbox_id)
                .or_default()
                .insert(location.uid);
        }

        let mut targets = Vec::with_capacity(uids_by_mailbox.len());
        for (mailbox_id, uids) in uids_by_mailbox {
            let mailbox = db
                .get_mailbox_by_id(&account.id, &mailbox_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Mailbox not found: {}", mailbox_id))?;
            targets.push((mailbox.name, uids.into_iter().collect::<Vec<_>>()));
        }

        (account, targets)
    };

    let mut session = mail_imap::connect_imap(&account)
        .await
        .map_err(|e| e.to_string())?;
    let store_cmd = if request.read {
        "+FLAGS.SILENT (\\Seen)"
    } else {
        "-FLAGS.SILENT (\\Seen)"
    };

    for (mailbox_name, uids) in mailbox_targets {
        session
            .select(&mailbox_name)
            .await
            .map_err(|e| e.to_string())?;
        for chunk in uids.chunks(250) {
            let sequence_set = format_uid_sequence_set(chunk);
            let mut updates = session
                .uid_store(&sequence_set, store_cmd)
                .await
                .map_err(|e| e.to_string())?;
            while updates
                .try_next()
                .await
                .map_err(|e| e.to_string())?
                .is_some()
            {}
        }
    }

    let _ = session.logout().await;

    let db = db.lock().await;
    db.set_threads_read_state(&thread_ids, request.read)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn list_attachments(
    message_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Attachment>, String> {
    let db = db.lock().await;
    db.get_message_attachments(&message_id)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn list_thread_attachments(
    thread_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Attachment>, String> {
    let db = db.lock().await;
    db.get_thread_attachments(&thread_id)
        .map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn open_attachment(id: String, db: State<'_, Arc<Mutex<Database>>>) -> Result<(), String> {
    let attachment = {
        let db = db.lock().await;
        db.get_attachment_by_id(&id)
            .map_err(|e: anyhow::Error| e.to_string())?
            .ok_or_else(|| "Attachment not found".to_string())?
    };

    if let Some(data) = attachment.data {
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(attachment.filename.unwrap_or_else(|| "unnamed".to_string()));
        std::fs::write(&file_path, data).map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", &file_path.to_string_lossy()])
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(&file_path)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(&file_path)
                .spawn();
        }
    } else {
        return Err("Attachment data not available locally. Downloading not yet implemented for large files.".to_string());
    }

    Ok(())
}

#[tauri::command]
pub async fn get_attachment_data(
    id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<u8>, String> {
    let db = db.lock().await;
    let att = db.get_attachment_by_id(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Attachment not found".to_string())?;

    att.data.ok_or_else(|| "No data for this attachment".to_string())
}

#[tauri::command]
pub async fn get_db_counts(
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<HashMap<String, i64>, String> {
    let db = db.lock().await;
    db.get_counts().map_err(|e: anyhow::Error| e.to_string())
}

#[tauri::command]
pub async fn delete_all_attachments(db: State<'_, Arc<Mutex<Database>>>) -> Result<(), String> {
    let db = db.lock().await;
    db.delete_all_attachments().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn wipe_local_data(
    reset_schema: Option<bool>,
    app: tauri::AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    search: State<'_, Arc<Mutex<SearchIndex>>>,
) -> Result<(), String> {
    let reset = reset_schema.unwrap_or(false);

    if reset {
        // Full file wipe is handled by deleting the files on next app start or via explicit deletion here
        // For simplicity, we drop all tables except accounts
        let mut db = db.lock().await;
        db.drop_tables().map_err(|e| e.to_string())?;
    } else {
        let db = db.lock().await;
        db.wipe_data().map_err(|e| e.to_string())?;
    }

    {
        let search = search.lock().await;
        search.clear_all().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_entire_mailbox(
    request: SyncAccountRequest,
    app: AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    search: State<'_, Arc<Mutex<SearchIndex>>>,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
) -> Result<SyncResult, String> {
    let account_id = request.account_id.clone();
    let mailbox_id = request.mailbox_id.clone();

    if mailbox_id.is_none() {
        return Err("A specific mailbox must be selected to fetch entire history".to_string());
    }

    {
        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr.lock().await;
        let key = format!("entire:{}:{}", account_id, mailbox_id.as_ref().unwrap());
        if mgr.is_syncing(&key) {
            return Ok(SyncResult {
                account_id,
                mailbox_id,
                new_messages: 0,
                error: Some("Full fetch already in progress".to_string()),
            });
        }
        mgr.start_sync(&key);
    }

    let db_clone = db.inner().clone();
    let search_clone = search.inner().clone();
    let sync_mgr_clone = sync_mgr.inner().clone();
    let app_clone = app.clone();
    let account_id_task = account_id.clone();
    let mailbox_id_task = mailbox_id.clone();

    tokio::spawn(async move {
        let result = async {
            let account = {
                let db = db_clone.lock().await;
                db.list_accounts()?
                    .into_iter()
                    .find(|a| a.id == account_id_task)
                    .ok_or_else(|| anyhow::anyhow!("Account not found"))?
            };

            let mut session = mail_imap::connect_imap(&account).await?;
            let mut total_new = 0;

            let mailbox = {
                let db = db_clone.lock().await;
                db.get_mailbox_by_id(&account.id, mailbox_id_task.as_ref().unwrap())?
                    .ok_or_else(|| anyhow::anyhow!("Mailbox not found"))?
            };

            let _ = app_clone.emit("sync-progress", SyncProgress {
                message: format!("Starting full fetch for {}…", mailbox.name),
                current: None,
                total: None,
            });
            let _select = session.select(&mailbox.name).await?;

            // Search for ALL messages
            let uids_set = session.uid_search("ALL").await?;
            let mut uids: Vec<u32> = uids_set.into_iter().collect();
            uids.sort_unstable_by(|a, b| b.cmp(a)); // Newest first

            if !uids.is_empty() {
                let total = uids.len();
                for (i, chunk) in uids.chunks(500).enumerate() {
                    let uid_range = format_uid_sequence_set(chunk);
                    let current_count = (i * 500) + chunk.len();
                    let _ = app_clone.emit("sync-progress", SyncProgress {
                        message: format!("Fetching messages {}-{} of {}…", i * 500, current_count, total),
                        current: Some(current_count),
                        total: Some(total),
                    });
                    let fetches: Vec<_> = session
                        .uid_fetch(&uid_range, "(BODY.PEEK[] FLAGS UID)")
                        .await?
                        .try_collect()
                        .await?;

                    let gmail_labels = if account.provider == "gmail" {
                        crate::mail::imap::fetch_gmail_vibemail_labels(&mut session, &uid_range).await?
                    } else {
                        HashMap::new()
                    };

                    let batch_results = crate::mail::imap::parse_fetches(&fetches, &account, &mailbox, &gmail_labels);
                    if !batch_results.is_empty() {
                        crate::mail::imap::persist_batch(&batch_results, &account, &mailbox, &db_clone).await?;
                        total_new += batch_results.len();
                    }
                }
            }

            let _ = session.logout().await;
            Ok::<usize, anyhow::Error>(total_new)
        }.await;

        let err = match result {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr_clone.lock().await;
        let key = format!("entire:{}:{}", account_id_task, mailbox_id_task.as_ref().unwrap());
        mgr.finish_sync(&key, err);
    });

    Ok(SyncResult {
        account_id,
        mailbox_id,
        new_messages: 0,
        error: None,
    })
}

#[tauri::command]
pub async fn move_message(
message_id: String, target_mailbox: String) -> Result<(), String> {
    tracing::info!("move_message: {} -> {}", message_id, target_mailbox);
    Ok(())
}
