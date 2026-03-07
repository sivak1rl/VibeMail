use crate::ai::{
    provider::{build_thread_context, ChatMessage},
    router::{AiRouter, TaskKind},
    stream::stream_to_frontend,
    tools::{
        parse_extracted_actions, parse_triage_score, SYSTEM_DRAFT, SYSTEM_EXTRACT,
        SYSTEM_SUMMARIZE, SYSTEM_TRIAGE,
    },
};
use crate::db::{
    models::{AiConfig, ExtractedAction},
    Database,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct AiThreadRequest {
    pub thread_id: String,
    pub account_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageResult {
    pub thread_id: String,
    pub score: f64,
}

#[tauri::command]
pub async fn summarize_thread(
    request: AiThreadRequest,
    app: AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    router: State<'_, Arc<Mutex<AiRouter>>>,
) -> Result<String, String> {
    let messages = {
        let db = db.lock().await;
        db.get_thread_messages(&request.thread_id)
            .map_err(|e| e.to_string())?
    };
    if messages.is_empty() {
        return Err("Thread not found".to_string());
    }

    let privacy = {
        let db = db.lock().await;
        db.get_ai_config().unwrap_or_default().privacy_mode
    };

    let context = build_thread_context(&messages, 6000, privacy);
    let chat_messages = vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM_SUMMARIZE.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!("Thread to summarize:\n\n{}", context),
        },
    ];

    let stream = {
        let router = router.lock().await;
        router
            .stream_complete(TaskKind::Summary, chat_messages)
            .await
            .map_err(|e| e.to_string())?
    };

    let event_name = format!("ai_summary_{}", request.thread_id);
    let summary = stream_to_frontend(&app, stream, &event_name)
        .await
        .map_err(|e| e.to_string())?;

    {
        let db = db.lock().await;
        let _ = db.update_thread_summary(&request.thread_id, &summary);
    }
    Ok(summary)
}

#[tauri::command]
pub async fn draft_reply(
    request: AiThreadRequest,
    app: AppHandle,
    db: State<'_, Arc<Mutex<Database>>>,
    router: State<'_, Arc<Mutex<AiRouter>>>,
) -> Result<String, String> {
    let messages = {
        let db = db.lock().await;
        db.get_thread_messages(&request.thread_id)
            .map_err(|e| e.to_string())?
    };
    if messages.is_empty() {
        return Err("Thread not found".to_string());
    }

    let privacy = {
        let db = db.lock().await;
        db.get_ai_config().unwrap_or_default().privacy_mode
    };

    let context = build_thread_context(&messages, 6000, privacy);
    let chat_messages = vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM_DRAFT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!("Email thread:\n\n{}", context),
        },
    ];

    let stream = {
        let router = router.lock().await;
        router
            .stream_complete(TaskKind::Draft, chat_messages)
            .await
            .map_err(|e| e.to_string())?
    };

    let event_name = format!("ai_draft_{}", request.thread_id);
    stream_to_frontend(&app, stream, &event_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_actions(
    request: AiThreadRequest,
    db: State<'_, Arc<Mutex<Database>>>,
    router: State<'_, Arc<Mutex<AiRouter>>>,
) -> Result<Vec<ExtractedAction>, String> {
    let messages = {
        let db = db.lock().await;
        db.get_thread_messages(&request.thread_id)
            .map_err(|e| e.to_string())?
    };
    if messages.is_empty() {
        return Err("Thread not found".to_string());
    }

    let privacy = {
        let db = db.lock().await;
        db.get_ai_config().unwrap_or_default().privacy_mode
    };

    let context = build_thread_context(&messages, 4000, privacy);
    let chat_messages = vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM_EXTRACT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!("Email thread:\n\n{}", context),
        },
    ];

    let response = {
        let router = router.lock().await;
        router
            .complete(TaskKind::Extract, chat_messages)
            .await
            .map_err(|e| e.to_string())?
    };

    parse_extracted_actions(&response).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn triage_thread(
    request: AiThreadRequest,
    db: State<'_, Arc<Mutex<Database>>>,
    router: State<'_, Arc<Mutex<AiRouter>>>,
) -> Result<TriageResult, String> {
    let messages = {
        let db = db.lock().await;
        db.get_thread_messages(&request.thread_id)
            .map_err(|e| e.to_string())?
    };
    if messages.is_empty() {
        return Err("Thread not found".to_string());
    }

    let first = &messages[0];
    let subject = first.subject.as_deref().unwrap_or("[no subject]");
    let sender = first
        .from
        .first()
        .map(|a| a.email.as_str())
        .unwrap_or("[unknown]");
    let snippet = first
        .body_text
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(500)
        .collect::<String>();

    let chat_messages = vec![
        ChatMessage {
            role: "system".into(),
            content: SYSTEM_TRIAGE.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!("From: {}\nSubject: {}\n\n{}", sender, subject, snippet),
        },
    ];

    let response = {
        let router = router.lock().await;
        router
            .complete(TaskKind::Triage, chat_messages)
            .await
            .map_err(|e| e.to_string())?
    };

    let score = parse_triage_score(&response);
    {
        let db = db.lock().await;
        for msg in &messages {
            let _ = db.update_message_triage(&msg.id, score);
        }
        let _ = db.update_thread_summary(&request.thread_id, "");
    }

    Ok(TriageResult {
        thread_id: request.thread_id,
        score,
    })
}

#[tauri::command]
pub async fn get_ai_config(db: State<'_, Arc<Mutex<Database>>>) -> Result<AiConfig, String> {
    let db = db.lock().await;
    db.get_ai_config().map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct SetAiConfigRequest {
    pub config: AiConfig,
    pub api_key: Option<String>,
}

#[tauri::command]
pub async fn set_ai_config(
    request: SetAiConfigRequest,
    db: State<'_, Arc<Mutex<Database>>>,
) -> Result<(), String> {
    if let Some(key) = &request.api_key {
        crate::auth::keychain::store_api_key("byok", key).map_err(|e| e.to_string())?;
    }
    let db = db.lock().await;
    db.set_ai_config(&request.config).map_err(|e| e.to_string())
}
