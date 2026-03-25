/// IMAP IDLE push notifications — per-account background tasks that hold
/// an IMAP connection in IDLE mode and emit events when new mail arrives.
use crate::db::{models::Account, Database};
use crate::mail::imap as mail_imap;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Mutex};
use tracing::{debug, info, warn};

/// Sent to the frontend when IDLE detects new mail.
#[derive(Clone, serde::Serialize)]
pub struct IdleNewMail {
    pub account_id: String,
    pub mailbox_name: String,
}

/// Per-account handle for controlling a running IDLE task.
struct IdleHandle {
    /// Send `false` to pause, `true` to resume, drop to shut down.
    pause_tx: watch::Sender<bool>,
    /// JoinHandle so we can await graceful shutdown.
    task: tokio::task::JoinHandle<()>,
}

/// Manages IDLE tasks for all accounts.
pub struct IdleManager {
    handles: HashMap<String, IdleHandle>,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }

    /// Start an IDLE listener for the given account. If one is already running, this is a no-op.
    pub fn start(&mut self, account: Account, db: Arc<Mutex<Database>>, app: AppHandle) {
        if self.handles.contains_key(&account.id) {
            debug!("IDLE already running for {}", account.id);
            return;
        }

        let (pause_tx, pause_rx) = watch::channel(true); // start active
        let account_id = account.id.clone();

        let task = tokio::spawn(idle_loop(account, pause_rx, db, app));

        self.handles
            .insert(account_id, IdleHandle { pause_tx, task });
    }

    /// Temporarily pause IDLE for an account (e.g. during manual sync).
    pub fn pause(&self, account_id: &str) {
        if let Some(h) = self.handles.get(account_id) {
            let _ = h.pause_tx.send(false);
        }
    }

    /// Resume IDLE after a pause.
    pub fn resume(&self, account_id: &str) {
        if let Some(h) = self.handles.get(account_id) {
            let _ = h.pause_tx.send(true);
        }
    }

    /// Stop IDLE for an account and wait for the task to finish.
    pub async fn stop(&mut self, account_id: &str) {
        if let Some(h) = self.handles.remove(account_id) {
            // Dropping pause_tx closes the channel, which the loop detects.
            drop(h.pause_tx);
            let _ = h.task.await;
            info!("IDLE stopped for {}", account_id);
        }
    }

    /// Stop all IDLE tasks (e.g. on app exit).
    pub async fn stop_all(&mut self) {
        let ids: Vec<String> = self.handles.keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }
}

impl Default for IdleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The main IDLE loop for one account. Connects, selects INBOX (or All Mail for
/// Gmail), enters IDLE, and waits for server notifications. On new mail it
/// triggers a lightweight sync and emits `idle-new-mail` to the frontend.
async fn idle_loop(
    account: Account,
    mut pause_rx: watch::Receiver<bool>,
    db: Arc<Mutex<Database>>,
    app: AppHandle,
) {
    let mut backoff = Duration::from_secs(5);
    let max_backoff = Duration::from_secs(300); // 5 min cap

    loop {
        // Check if the channel is closed (manager dropped the sender → shutdown).
        if pause_rx.has_changed().is_err() {
            info!("IDLE channel closed for {}, shutting down", account.id);
            return;
        }

        // Wait until we're not paused.
        loop {
            if *pause_rx.borrow() {
                break; // active
            }
            // Wait for a change — if the channel closes, shut down.
            if pause_rx.changed().await.is_err() {
                info!(
                    "IDLE channel closed for {} during pause, shutting down",
                    account.id
                );
                return;
            }
        }

        info!("IDLE connecting for {}", account.id);

        match run_idle_session(&account, &mut pause_rx, &db, &app).await {
            Ok(()) => {
                // Clean exit (paused or shutdown) — reset backoff
                backoff = Duration::from_secs(5);
            }
            Err(e) => {
                warn!(
                    "IDLE error for {}: {}. Retrying in {:?}",
                    account.id, e, backoff
                );
                // Wait the backoff period, but abort early if the channel closes.
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    _ = pause_rx.changed() => {
                        // Could be pause, resume, or close — loop will handle it.
                    }
                }
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

/// One IDLE session: connect, select mailbox, loop IDLE with 25-min re-issue.
async fn run_idle_session(
    account: &Account,
    pause_rx: &mut watch::Receiver<bool>,
    db: &Arc<Mutex<Database>>,
    app: &AppHandle,
) -> Result<()> {
    let mut session = mail_imap::connect_imap_with_retry(account).await?;

    // Pick the right mailbox — Gmail uses [Gmail]/All Mail, others use INBOX.
    let mailbox_name = if account.provider == "gmail" {
        let db_lock = db.lock().await;
        let mailboxes = db_lock.list_mailboxes(&account.id)?;
        drop(db_lock);
        mailboxes
            .iter()
            .find(|mb| mb.name.to_lowercase() == "[gmail]/all mail")
            .map(|mb| mb.name.clone())
            .unwrap_or_else(|| "INBOX".to_string())
    } else {
        "INBOX".to_string()
    };

    session.select(&mailbox_name).await?;
    info!("IDLE selected {} for {}", mailbox_name, account.id);

    // Re-issue IDLE every 25 minutes (RFC 2177 recommends < 29 min).
    let idle_timeout = Duration::from_secs(25 * 60);

    loop {
        // Check for pause/shutdown before entering IDLE.
        if pause_rx.has_changed().is_err() {
            let _ = session.logout().await;
            return Ok(());
        }
        if !*pause_rx.borrow() {
            let _ = session.logout().await;
            return Ok(()); // paused — outer loop will wait and reconnect
        }

        debug!("IDLE entering IDLE for {} on {}", account.id, mailbox_name);

        let mut idle_handle = session.idle();
        idle_handle.init().await?;

        // wait_with_timeout returns (Future, StopSource). We run the future
        // in a select against pause/shutdown. If we need to interrupt early,
        // dropping the StopSource cancels the wait.
        let idle_result = {
            let (wait_future, stop_source) = idle_handle.wait_with_timeout(idle_timeout);
            tokio::pin!(wait_future);

            tokio::select! {
                result = &mut wait_future => {
                    drop(stop_source);
                    Some(result)
                }
                result = pause_rx.changed() => {
                    // Drop stop_source to interrupt the IDLE wait future.
                    drop(stop_source);
                    if result.is_err() || !*pause_rx.borrow() {
                        // Shutdown or pause requested.
                        None
                    } else {
                        None
                    }
                }
            }
        };
        // wait_future is now dropped, so idle_handle is no longer borrowed.

        match idle_result {
            None => {
                // Pause or shutdown — exit cleanly.
                session = idle_handle.done().await?;
                let _ = session.logout().await;
                return Ok(());
            }
            Some(Err(e)) => {
                let _ = idle_handle.done().await;
                return Err(anyhow::anyhow!("IDLE wait error: {}", e));
            }
            Some(Ok(reason)) => {
                let got_data =
                    !matches!(reason, async_imap::extensions::idle::IdleResponse::Timeout);
                session = idle_handle.done().await?;

                if got_data {
                    info!(
                        "IDLE detected new mail for {} in {}",
                        account.id, mailbox_name
                    );
                    let _ = app.emit(
                        "idle-new-mail",
                        IdleNewMail {
                            account_id: account.id.clone(),
                            mailbox_name: mailbox_name.clone(),
                        },
                    );
                    // Re-select to refresh mailbox state.
                    session.select(&mailbox_name).await?;
                }
            }
        }
    }
}
