use crate::db::{models::Draft, Database};
use crate::mail::imap as mail_imap;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct SaveDraftRequest {
    pub id: String,
    pub account_id: Option<String>,
    pub mode: String,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DraftResponse {
    pub id: String,
    pub account_id: Option<String>,
    pub mode: String,
    pub to_addrs: String,
    pub cc_addrs: String,
    pub bcc_addrs: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub thread_id: Option<String>,
    pub updated_at: i64,
}

impl From<Draft> for DraftResponse {
    fn from(d: Draft) -> Self {
        Self {
            id: d.id,
            account_id: d.account_id,
            mode: d.mode,
            to_addrs: d.to_addrs,
            cc_addrs: d.cc_addrs,
            bcc_addrs: d.bcc_addrs,
            subject: d.subject,
            body_text: d.body_text,
            body_html: d.body_html,
            in_reply_to: d.in_reply_to,
            thread_id: d.thread_id,
            updated_at: d.updated_at,
        }
    }
}

#[tauri::command]
pub async fn save_draft(
    request: SaveDraftRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let draft = Draft {
        id: request.id,
        account_id: request.account_id,
        mode: request.mode,
        to_addrs: request.to_addrs,
        cc_addrs: request.cc_addrs,
        bcc_addrs: request.bcc_addrs,
        subject: request.subject,
        body_text: request.body_text,
        body_html: request.body_html,
        in_reply_to: request.in_reply_to,
        thread_id: request.thread_id,
        updated_at: 0, // set by DB
    };
    let db = db.lock().await;
    db.save_draft(&draft).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_draft(
    id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Option<DraftResponse>, String> {
    let db = db.lock().await;
    db.get_draft(&id)
        .map(|opt| opt.map(DraftResponse::from))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_draft(id: String, db: State<'_, Arc<Mutex<Database>>>) -> Result<(), String> {
    let db = db.lock().await;
    db.delete_draft(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_drafts(
    account_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<DraftResponse>, String> {
    let db = db.lock().await;
    db.list_drafts(&account_id)
        .map(|drafts| drafts.into_iter().map(DraftResponse::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn count_drafts(
    account_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<u32, String> {
    let db = db.lock().await;
    db.count_drafts(&account_id).map_err(|e| e.to_string())
}

/// Save a draft to the server's Drafts folder via IMAP APPEND.
/// If the draft was previously synced (has an imap_uid stored locally),
/// the old server copy is deleted first (IMAP drafts are immutable).
#[tauri::command]
pub async fn sync_draft_to_imap(
    id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let (draft, account, drafts_folder) = {
        let db = db.lock().await;
        let draft = db
            .get_draft(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Draft not found: {}", id))?;
        let account_id = draft
            .account_id
            .as_deref()
            .ok_or("Draft has no account_id")?;
        let account = db
            .list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| "Account not found".to_string())?;
        let drafts_folder = db
            .get_mailbox_by_role(account_id, "drafts")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "No drafts folder found for this account".to_string())?;
        (draft, account, drafts_folder.name)
    };

    // Build RFC 2822 message from the draft
    let rfc2822 =
        build_draft_rfc2822(&account.email, &account.name, &draft).map_err(|e| e.to_string())?;

    let mut session = mail_imap::connect_imap_with_retry(&account)
        .await
        .map_err(|e| e.to_string())?;

    // APPEND to the Drafts folder with \Draft and \Seen flags
    session
        .append(&drafts_folder, Some("(\\Draft \\Seen)"), None, &rfc2822)
        .await
        .map_err(|e| format!("IMAP APPEND failed: {}", e))?;

    session.logout().await.ok();
    Ok(())
}

/// Delete the server-side copy of a draft from the IMAP Drafts folder.
/// Searches by Message-ID header to find the UID, then flags \Deleted + EXPUNGE.
#[tauri::command]
pub async fn delete_draft_from_imap(
    id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let (draft, account, drafts_folder) = {
        let db = db.lock().await;
        let draft = match db.get_draft(&id).map_err(|e| e.to_string())? {
            Some(d) => d,
            None => return Ok(()), // Already deleted locally, nothing to clean up
        };
        let account_id = match draft.account_id.as_deref() {
            Some(id) => id,
            None => return Ok(()),
        };
        let account = db
            .list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| "Account not found".to_string())?;
        let drafts_folder = match db
            .get_mailbox_by_role(account_id, "drafts")
            .map_err(|e| e.to_string())?
        {
            Some(mb) => mb.name,
            None => return Ok(()), // No drafts folder — nothing to delete
        };
        (draft, account, drafts_folder)
    };

    // Use the draft ID as a synthetic Message-ID to find the server copy
    let search_msgid = format!("vibemail-draft-{}", draft.id);

    let mut session = mail_imap::connect_imap_with_retry(&account)
        .await
        .map_err(|e| e.to_string())?;

    session
        .select(&drafts_folder)
        .await
        .map_err(|e| format!("Failed to select drafts folder: {}", e))?;

    // Search for the message by Message-ID header
    let uids = session
        .uid_search(format!("HEADER Message-ID \"<{}>\"", search_msgid))
        .await
        .map_err(|e| format!("IMAP SEARCH failed: {}", e))?;

    if !uids.is_empty() {
        let uid_list: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
        let uid_set = uid_list.join(",");
        {
            let mut updates = session
                .uid_store(&uid_set, "+FLAGS.SILENT (\\Deleted)")
                .await
                .map_err(|e| format!("IMAP STORE failed: {}", e))?;
            while updates
                .try_next()
                .await
                .map_err(|e| format!("IMAP STORE stream: {}", e))?
                .is_some()
            {}
        }
        let expunged = session
            .expunge()
            .await
            .map_err(|e| format!("IMAP EXPUNGE failed: {}", e))?;
        futures::pin_mut!(expunged);
        while expunged
            .try_next()
            .await
            .map_err(|e| format!("IMAP EXPUNGE stream: {}", e))?
            .is_some()
        {}
    }

    session.logout().await.ok();
    Ok(())
}

/// Build an RFC 2822 message from a Draft for IMAP APPEND.
fn build_draft_rfc2822(
    from_email: &str,
    from_name: &str,
    draft: &Draft,
) -> anyhow::Result<Vec<u8>> {
    use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
    use lettre::Message;

    let from: Mailbox = format!("{} <{}>", from_name, from_email).parse()?;
    let mut builder = Message::builder()
        .from(from)
        .subject(&draft.subject)
        .message_id(Some(format!(
            "<vibemail-draft-{}@vibemail.local>",
            draft.id
        )));

    // Parse To addresses (comma-separated)
    for addr in draft
        .to_addrs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Ok(mb) = addr.parse::<Mailbox>() {
            builder = builder.to(mb);
        }
    }

    // Parse CC addresses
    for addr in draft
        .cc_addrs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Ok(mb) = addr.parse::<Mailbox>() {
            builder = builder.cc(mb);
        }
    }

    if let Some(irt) = &draft.in_reply_to {
        builder = builder.in_reply_to(irt.to_string());
    }

    let body = if let Some(html) = &draft.body_html {
        MultiPart::alternative()
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(draft.body_text.clone()),
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
                .body(draft.body_text.clone()),
        )
    };

    let email = builder.multipart(body)?;
    Ok(email.formatted())
}
