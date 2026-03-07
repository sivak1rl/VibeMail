use crate::db::{models::*, Database};
use crate::mail::{imap as mail_imap, sync::SyncManager};
use crate::search::SearchIndex;
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
    {
        let mut mgr = sync_mgr.lock().await;
        if mgr.is_syncing(&account_id) {
            return Ok(SyncResult {
                account_id,
                mailbox_id: request.mailbox_id.clone(),
                new_messages: 0,
                error: Some("Sync already in progress".to_string()),
            });
        }
        mgr.start_sync(&account_id);
    }

    let result = do_sync(
        &account_id,
        request.mailbox_id.as_deref(),
        db.inner().clone(),
        search.inner().clone(),
        app,
    )
    .await;

    let (new_count, err) = match &result {
        Ok(n) => (*n, None),
        Err(e) => (0, Some(e.to_string())),
    };

    {
        let mut mgr = sync_mgr.lock().await;
        mgr.finish_sync(&account_id, err.clone());
    }

    Ok(SyncResult {
        account_id,
        mailbox_id: request.mailbox_id,
        new_messages: new_count,
        error: err,
    })
}

async fn do_sync(
    account_id: &str,
    mailbox_id: Option<&str>,
    db: Arc<Mutex<Database>>,
    search: Arc<Mutex<SearchIndex>>,
    app: AppHandle,
) -> anyhow::Result<usize> {
    let account = {
        let db = db.lock().await;
        db.list_accounts()?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?
    };

    let mut session = mail_imap::connect_imap(&account).await?;

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
                })
        }
    };

    let _ = app.emit("sync-progress", "Connecting to IMAP…");

    let messages = mail_imap::sync_mailbox(&mut session, &account, &mut mailbox, db.clone(), {
        let app = app.clone();
        move |status: &str| {
            let _ = app.emit("sync-progress", status);
        }
    })
    .await?;
    let count = messages.len();

    let _ = app.emit("sync-progress", format!("Indexing {} messages…", count));

    {
        let search = search.lock().await;
        for msg in &messages {
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
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_read(message_id: String) -> Result<(), String> {
    tracing::info!("mark_read: {}", message_id);
    Ok(())
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
        .map_err(|e| e.to_string())
}

fn format_uid_sequence_set(uids: &[u32]) -> String {
    if uids.is_empty() {
        return String::new();
    }

    let mut ranges = Vec::new();
    let mut start = uids[0];
    let mut prev = uids[0];
    for &uid in &uids[1..] {
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

#[tauri::command]
pub async fn move_message(message_id: String, target_mailbox: String) -> Result<(), String> {
    tracing::info!("move_message: {} -> {}", message_id, target_mailbox);
    Ok(())
}
