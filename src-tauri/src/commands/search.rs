use crate::ai::router::{AiRouter, TaskKind};
use crate::db::{models::Thread, Database};
use crate::mail::sync::SyncManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub account_id: String,
    pub mailbox_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[tauri::command]
pub async fn search_messages(
    request: SearchRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<Vec<Thread>, String> {
    let limit = request.limit.unwrap_or(20);
    let offset = request.offset.unwrap_or(0);
    let db = db.lock().await;
    let thread_ids = db
        .fts_search(
            &request.query,
            &request.account_id,
            request.mailbox_id.as_deref(),
            limit,
            offset,
        )
        .map_err(|e| e.to_string())?;
    db.get_threads_by_ids(&thread_ids, None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_semantic(
    request: SearchRequest,
    db: State<'_, Arc<Mutex<Database>>>,
    ai: State<'_, Arc<AiRouter>>,
) -> Result<Vec<Thread>, String> {
    let limit = request.limit.unwrap_or(20) as usize;
    let offset = request.offset.unwrap_or(0) as usize;
    tracing::info!("Semantic search for: \"{}\"", request.query);

    // 1. Generate embedding for query
    let query_embedding = ai.embed(&request.query).await.map_err(|e| {
        tracing::error!("Failed to generate query embedding: {}", e);
        e.to_string()
    })?;
    let model = ai
        .model_for(&TaskKind::Embed)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Using embedding model: {}", model);

    // 2. Search database for closest threads
    let db = db.lock().await;
    let matches = db
        .semantic_search(&request.account_id, &query_embedding, &model, limit, offset)
        .map_err(|e| {
            tracing::error!("Database semantic search failed: {}", e);
            e.to_string()
        })?;

    tracing::info!("Found {} semantic matches", matches.len());
    let thread_ids: Vec<String> = matches.into_iter().map(|(id, _)| id).collect();

    // 3. Hydrate threads (Global search)
    let threads = db
        .get_threads_by_ids(&thread_ids, None)
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "Hydrated {} threads from {} IDs",
        threads.len(),
        thread_ids.len()
    );

    Ok(threads)
}

#[tauri::command]
pub async fn reindex_all_semantic(
    account_id: String,
    db: State<'_, Arc<Mutex<Database>>>,
    ai: State<'_, Arc<AiRouter>>,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let account_id_task = account_id.clone();
    let db_clone = db.inner().clone();
    let ai_clone = ai.inner().clone();
    let sync_mgr_clone = sync_mgr.inner().clone();
    let app_clone = app.clone();

    {
        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr.lock().await;
        if mgr.is_syncing(&format!("reindex:{}", account_id)) {
            return Err("Reindexing already in progress".to_string());
        }
        mgr.start_sync(&format!("reindex:{}", account_id));
    }

    tokio::spawn(async move {
        use tauri::Emitter;
        tracing::info!("Starting reindex for {}", account_id_task);

        let result = async {
            let model = ai_clone.model_for(&TaskKind::Embed).await.map_err(|e| {
                let err = format!("Failed to get embedding model: {}", e);
                tracing::error!("{}", err);
                err
            })?;

            tracing::info!("Using embedding model {}", model);

            let threads = {
                let db = db_clone.lock().await;
                db.list_threads(&account_id_task, None, 50000, 0)
                    .map_err(|e| {
                        let err = format!("Failed to list threads: {}", e);
                        tracing::error!("{}", err);
                        err
                    })?
            };

            let total = threads.len();
            tracing::info!("Reindex: found {} threads to embed", total);

            if total == 0 {
                return Ok(0);
            }

            let mut count = 0;
            let mut success_count = 0;

            for thread in threads {
                count += 1;
                if total < 50 || count % 10 == 0 || count == total {
                    let _ = app_clone.emit(
                        "reindex-progress",
                        format!("Embedding thread {} of {}…", count, total),
                    );
                }

                let messages = {
                    let db = db_clone.lock().await;
                    db.get_thread_messages(&thread.id)
                        .map_err(|e| e.to_string())?
                };

                if messages.is_empty() {
                    continue;
                }

                let body = messages[0]
                    .body_text
                    .as_deref()
                    .or(messages[0].body_html.as_deref())
                    .unwrap_or("");
                let body_truncated: String = body.chars().take(2000).collect();

                let context = format!(
                    "Subject: {}\n\n{}",
                    thread.subject.as_deref().unwrap_or("(no subject)"),
                    body_truncated
                );

                match ai_clone.embed(&context).await {
                    Ok(embedding) => {
                        let db = db_clone.lock().await;
                        if let Err(e) = db.upsert_thread_embedding(&thread.id, &model, &embedding) {
                            tracing::error!("Reindex: upsert failed: {}", e);
                        } else {
                            success_count += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Reindex: embed failed: {}", e);
                    }
                }
            }

            tracing::info!("Reindex finished. Indexed {} threads", success_count);
            Ok::<usize, String>(success_count)
        }
        .await;

        let err = result.err();

        let mut mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr_clone.lock().await;
        mgr.finish_sync(&format!("reindex:{}", account_id_task), err);
    });

    Ok(())
}

#[tauri::command]
pub async fn get_reindex_status(
    account_id: String,
    sync_mgr: State<'_, Arc<Mutex<SyncManager>>>,
) -> Result<bool, String> {
    let mgr: tokio::sync::MutexGuard<'_, SyncManager> = sync_mgr.lock().await;
    Ok(mgr.is_syncing(&format!("reindex:{}", account_id)))
}
