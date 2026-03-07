mod ai;
mod auth;
mod commands;
mod db;
mod mail;
mod search;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
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

            let ai_router = Arc::new(tokio::sync::Mutex::new(ai::router::AiRouter::new(
                db.clone(),
            )));

            app.manage(db);
            app.manage(search);
            app.manage(ai_router);
            app.manage(Arc::new(tokio::sync::Mutex::new(
                mail::sync::SyncManager::new(),
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
            commands::imap::list_mailboxes,
            commands::imap::list_threads,
            commands::imap::get_thread,
            commands::imap::mark_read,
            commands::imap::set_threads_read,
            commands::imap::set_threads_flagged,
            commands::imap::archive_threads,
            commands::imap::move_message,
            commands::smtp::send_message,
            commands::ai::summarize_thread,
            commands::ai::draft_reply,
            commands::ai::extract_actions,
            commands::ai::triage_thread,
            commands::ai::categorize_threads,
            commands::ai::get_ai_config,
            commands::ai::get_thread_insights,
            commands::ai::set_ai_config,
            commands::search::search_messages,
            commands::search::search_semantic,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
