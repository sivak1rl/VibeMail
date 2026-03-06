/// Background sync manager - tracks per-account sync state
use std::collections::HashMap;
use std::time::Instant;

pub struct SyncState {
    pub last_sync: Option<Instant>,
    pub is_syncing: bool,
    pub error: Option<String>,
}

pub struct SyncManager {
    accounts: HashMap<String, SyncState>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn start_sync(&mut self, account_id: &str) {
        let state = self.accounts.entry(account_id.to_string()).or_insert(SyncState {
            last_sync: None,
            is_syncing: false,
            error: None,
        });
        state.is_syncing = true;
        state.error = None;
    }

    pub fn finish_sync(&mut self, account_id: &str, error: Option<String>) {
        if let Some(state) = self.accounts.get_mut(account_id) {
            state.is_syncing = false;
            state.last_sync = Some(Instant::now());
            state.error = error;
        }
    }

    pub fn is_syncing(&self, account_id: &str) -> bool {
        self.accounts
            .get(account_id)
            .map(|s| s.is_syncing)
            .unwrap_or(false)
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}
