use crate::ai::{
    provider::{build_thread_context, ChatMessage},
    router::{AiRouter, TaskKind},
    stream::stream_to_frontend,
    tools::{
        build_categorize_system_prompt, parse_category_label, parse_extracted_actions,
        parse_triage_score, SYSTEM_DRAFT, SYSTEM_EXTRACT, SYSTEM_SUMMARIZE, SYSTEM_TRIAGE,
    },
};
use crate::db::{
    models::{AiConfig, ExtractedAction},
    Database,
};
use crate::mail::imap as mail_imap;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CategorizeThreadsRequest {
    pub thread_ids: Vec<String>,
    pub custom_categories: Option<Vec<CustomCategoryInput>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CategorizeThreadResult {
    pub thread_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCategoryInput {
    pub name: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PromptCategory {
    name: String,
    examples: Vec<String>,
}

#[derive(Debug, Clone)]
struct PlannedCategoryUpdate {
    account_id: String,
    thread_id: String,
    label: String,
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
pub async fn categorize_threads(
    request: CategorizeThreadsRequest,
    db: State<'_, Arc<Mutex<Database>>>,
    router: State<'_, Arc<Mutex<AiRouter>>>,
) -> Result<Vec<CategorizeThreadResult>, String> {
    let accounts_by_id = {
        let db = db.lock().await;
        db.list_accounts()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|account| (account.id.clone(), account))
            .collect::<HashMap<_, _>>()
    };

    let categories = build_safe_categories(request.custom_categories);
    let allowed_labels = categories
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>();
    let schema_json = serde_json::to_string(&categories).map_err(|e| e.to_string())?;
    let system_prompt = build_categorize_system_prompt(&schema_json);

    let thread_ids = request
        .thread_ids
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if thread_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut planned_updates = Vec::with_capacity(thread_ids.len());
    for thread_id in thread_ids {
        let messages = {
            let db = db.lock().await;
            db.get_thread_messages(&thread_id)
                .map_err(|e| e.to_string())?
        };
        if messages.is_empty() {
            continue;
        }

        let privacy = {
            let db = db.lock().await;
            db.get_ai_config().unwrap_or_default().privacy_mode
        };
        let context = build_thread_context(&messages, 2500, privacy);
        let chat_messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt.clone(),
            },
            ChatMessage {
                role: "user".into(),
                content: format!("Thread:\n\n{}", context),
            },
        ];

        let response = {
            let router = router.lock().await;
            router
                .complete(TaskKind::Extract, chat_messages)
                .await
                .map_err(|e| e.to_string())?
        };
        let label = parse_category_label(&response, &allowed_labels);
        let account_id = messages[0].account_id.clone();

        planned_updates.push(PlannedCategoryUpdate {
            account_id,
            thread_id,
            label,
        });
    }

    let mut updates_by_account: HashMap<String, Vec<PlannedCategoryUpdate>> = HashMap::new();
    for update in &planned_updates {
        updates_by_account
            .entry(update.account_id.clone())
            .or_default()
            .push(update.clone());
    }

    for (account_id, updates) in &updates_by_account {
        let account = accounts_by_id
            .get(account_id)
            .ok_or_else(|| format!("Account not found: {}", account_id))?;
        if account.provider == "gmail" {
            sync_gmail_category_labels_for_account(
                account,
                updates,
                &allowed_labels,
                db.inner().clone(),
            )
            .await?;
        }
    }

    let mut categorized = Vec::with_capacity(planned_updates.len());
    for update in planned_updates {
        {
            let db = db.lock().await;
            let existing = db
                .get_threads_by_ids(std::slice::from_ref(&update.thread_id), None)
                .map_err(|e| e.to_string())?
                .into_iter()
                .next()
                .map(|thread| thread.labels)
                .unwrap_or_default();

            let mut labels = existing
                .into_iter()
                .filter(|value| !allowed_labels.iter().any(|allowed| allowed == value))
                .collect::<Vec<_>>();
            labels.push(update.label.clone());
            db.update_thread_labels(&update.thread_id, &labels)
                .map_err(|e| e.to_string())?;
        }

        categorized.push(CategorizeThreadResult {
            thread_id: update.thread_id,
            label: update.label,
        });
    }

    Ok(categorized)
}

fn build_safe_categories(custom: Option<Vec<CustomCategoryInput>>) -> Vec<PromptCategory> {
    let mut categories = vec![
        PromptCategory {
            name: "newsletter".to_string(),
            examples: vec![
                "weekly digest".to_string(),
                "product announcements".to_string(),
            ],
        },
        PromptCategory {
            name: "receipt".to_string(),
            examples: vec![
                "order confirmation".to_string(),
                "payment receipt".to_string(),
            ],
        },
        PromptCategory {
            name: "social".to_string(),
            examples: vec!["friend update".to_string(), "community message".to_string()],
        },
        PromptCategory {
            name: "updates".to_string(),
            examples: vec![
                "system notification".to_string(),
                "account status update".to_string(),
            ],
        },
    ];

    if let Some(custom_categories) = custom {
        for item in custom_categories.into_iter().take(12) {
            let name = sanitize_category_name(&item.name);
            if name.is_empty() || categories.iter().any(|c| c.name == name) {
                continue;
            }
            let mut examples = item
                .examples
                .into_iter()
                .filter_map(|e| sanitize_example(&e))
                .take(6)
                .collect::<Vec<_>>();
            if examples.is_empty() {
                examples.push("custom thread type".to_string());
            }
            categories.push(PromptCategory { name, examples });
        }
    }

    categories
}

fn sanitize_category_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .collect::<String>()
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .take(32)
        .collect()
}

fn sanitize_example(raw: &str) -> Option<String> {
    let cleaned = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(120)
        .collect::<String>();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

async fn sync_gmail_category_labels_for_account(
    account: &crate::db::models::Account,
    updates: &[PlannedCategoryUpdate],
    allowed_labels: &[String],
    db: Arc<Mutex<Database>>,
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }

    let mut session = mail_imap::connect_imap(account)
        .await
        .map_err(|e| e.to_string())?;
    let remove_arg = format_gmail_label_list(allowed_labels);

    for update in updates {
        let targets = {
            let db = db.lock().await;
            let locations = db
                .get_thread_message_locations(std::slice::from_ref(&update.thread_id))
                .map_err(|e| e.to_string())?;
            if locations.is_empty() {
                continue;
            }

            let mut uids_by_mailbox: HashMap<String, BTreeSet<u32>> = HashMap::new();
            for location in locations {
                uids_by_mailbox
                    .entry(location.mailbox_id)
                    .or_default()
                    .insert(location.uid);
            }

            let mut targets = Vec::new();
            for (mailbox_id, uids) in uids_by_mailbox {
                let mailbox = db
                    .get_mailbox_by_id(&account.id, &mailbox_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("Mailbox not found: {}", mailbox_id))?;
                targets.push((mailbox.name, uids.into_iter().collect::<Vec<_>>()));
            }
            targets
        };

        let add_arg = format_gmail_label_list(&[update.label.clone()]);
        for (mailbox_name, uids) in targets {
            session
                .select(&mailbox_name)
                .await
                .map_err(|e| e.to_string())?;
            for chunk in uids.chunks(250) {
                let sequence_set = format_uid_sequence_set(chunk);
                if sequence_set.is_empty() {
                    continue;
                }

                let remove_cmd = format!("-X-GM-LABELS.SILENT {}", remove_arg);
                let mut removed = session
                    .uid_store(&sequence_set, &remove_cmd)
                    .await
                    .map_err(|e| e.to_string())?;
                while removed
                    .try_next()
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some()
                {}
                drop(removed);

                let add_cmd = format!("+X-GM-LABELS.SILENT {}", add_arg);
                let mut added = session
                    .uid_store(&sequence_set, &add_cmd)
                    .await
                    .map_err(|e| e.to_string())?;
                while added.try_next().await.map_err(|e| e.to_string())?.is_some() {}
            }
        }
    }

    let _ = session.logout().await;
    Ok(())
}

fn format_gmail_label_list(labels: &[String]) -> String {
    let quoted = labels
        .iter()
        .map(|label| format!("\"{}\"", format_gmail_label_name(label)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("({})", quoted)
}

fn format_gmail_label_name(label: &str) -> String {
    let escaped = label.replace('"', "");
    format!("VibeMail/{}", escaped)
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
