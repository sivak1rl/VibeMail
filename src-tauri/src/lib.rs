mod ai;
mod auth;
mod commands;
mod db;
mod mail;
mod search;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Manager, RunEvent};
use tokio::sync::{oneshot, Mutex};
use tracing_subscriber::{fmt, EnvFilter};

/// Everything needed to complete an in-flight OAuth exchange, kept in memory.
pub struct OAuthFlow {
    pub rx: oneshot::Receiver<Result<auth::oauth::OAuthCallback, String>>,
    pub code_verifier: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub provider: String,
}

pub type OAuthPending = Arc<Mutex<HashMap<String, OAuthFlow>>>;

pub fn run() {
    fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vibemail=info,warn")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("vibemail.db");
            let db = db::Database::open(&db_path)?;
            let db = Arc::new(tokio::sync::Mutex::new(db));

            let search_dir = app_data_dir.join("search_index");
            let search = search::SearchIndex::open(&search_dir)?;
            let search = Arc::new(tokio::sync::Mutex::new(search));

            let ai_router = Arc::new(ai::router::AiRouter::new(db.clone()));

            app.manage(db);
            app.manage(search);
            app.manage(ai_router);
            app.manage(Arc::new(tokio::sync::Mutex::new(
                mail::sync::SyncManager::new(),
            )));
            app.manage(Arc::new(tokio::sync::Mutex::new(
                mail::idle::IdleManager::new(),
            )));
            let oauth_pending: OAuthPending = Arc::new(Mutex::new(HashMap::new()));
            app.manage(oauth_pending);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::accounts::add_account,
            commands::accounts::list_accounts,
            commands::accounts::remove_account,
            commands::accounts::get_oauth_url,
            commands::accounts::await_oauth_redirect,
            commands::accounts::complete_oauth,
            commands::imap::sync_account,
            commands::imap::fetch_history,
            commands::imap::fetch_entire_mailbox,
            commands::imap::list_mailboxes,
            commands::imap::list_threads,
            commands::imap::get_thread,
            commands::imap::get_sync_status,
            commands::imap::get_db_counts,
            commands::imap::mark_read,
            commands::imap::set_threads_read,
            commands::imap::set_threads_flagged,
            commands::imap::archive_threads,
            commands::imap::move_message,
            commands::imap::list_attachments,
            commands::imap::list_thread_attachments,
            commands::imap::open_attachment,
            commands::imap::get_attachment_data,
            commands::imap::delete_all_attachments,
            commands::imap::wipe_local_data,
            commands::imap::start_idle,
            commands::imap::stop_idle,
            commands::general::open_url,
            commands::smtp::send_message,
            commands::drafts::save_draft,
            commands::drafts::get_draft,
            commands::drafts::delete_draft,
            commands::ai::suggest_replies,
            commands::ai::summarize_thread,
            commands::ai::draft_reply,
            commands::ai::draft_new,
            commands::ai::proofread_text,
            commands::ai::extract_actions,
            commands::ai::triage_thread,
            commands::ai::categorize_threads,
            commands::ai::get_ai_config,
            commands::ai::get_thread_insights,
            commands::ai::set_ai_config,
            commands::ai::generate_roundup,
            commands::search::search_messages,
            commands::search::search_semantic,
            commands::search::reindex_all_semantic,
            commands::search::get_reindex_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Stop all IDLE tasks on app exit.
                let idle_mgr: Arc<Mutex<mail::idle::IdleManager>> =
                    app.state::<Arc<Mutex<mail::idle::IdleManager>>>().inner().clone();
                tauri::async_runtime::block_on(async {
                    let mut idle = idle_mgr.lock().await;
                    idle.stop_all().await;
                });
            }
        });
}
