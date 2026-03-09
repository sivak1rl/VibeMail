use crate::db::{models::Draft, Database};
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
pub async fn delete_draft(
    id: String,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let db = db.lock().await;
    db.delete_draft(&id).map_err(|e| e.to_string())
}
