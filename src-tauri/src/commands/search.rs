use crate::db::{models::Thread, Database};
use crate::search::SearchIndex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub account_id: String,
    pub limit: Option<u32>,
}

#[tauri::command]
pub async fn search_messages(
    request: SearchRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Thread>, String> {
    let limit = request.limit.unwrap_or(20);
    let db = db.lock().await;
    let thread_ids = db
        .fts_search(&request.query, &request.account_id, limit)
        .map_err(|e| e.to_string())?;
    db.get_threads_by_ids(&thread_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_semantic(
    request: SearchRequest,
    search: State<'_, Arc<Mutex<SearchIndex>>>,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Thread>, String> {
    let limit = request.limit.unwrap_or(20) as usize;
    let thread_ids = {
        let search = search.lock().await;
        search.search(&request.query, limit).map_err(|e| e.to_string())?
    };
    let db = db.lock().await;
    db.get_threads_by_ids(&thread_ids).map_err(|e| e.to_string())
}
