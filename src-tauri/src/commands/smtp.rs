use crate::db::{models::ComposeMessage, Database};
use crate::mail::smtp;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn send_message(
    message: ComposeMessage,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let account = {
        let db = db.lock().await;
        db.list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|a| a.id == message.account_id)
            .ok_or_else(|| "Account not found".to_string())?
    };

    smtp::send_message(&account, &message)
        .await
        .map_err(|e| e.to_string())
}
