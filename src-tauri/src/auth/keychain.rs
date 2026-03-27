use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn store() -> &'static Mutex<HashMap<String, String>> {
    STORE.get_or_init(|| Mutex::new(load_from_disk().unwrap_or_default()))
}

fn tokens_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/com.vibemail.app/tokens.json")
}

fn load_from_disk() -> Result<HashMap<String, String>> {
    let path = tokens_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_to_disk(map: &HashMap<String, String>) -> Result<()> {
    let path = tokens_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string(map)?)?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

pub fn store_token(account_id: &str, key: &str, value: &str) -> Result<()> {
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(format!("{}:{}", account_id, key), value.to_string());
    save_to_disk(&map)
}

pub fn get_token(account_id: &str, key: &str) -> Result<Option<String>> {
    let map = store().lock().unwrap_or_else(|e| e.into_inner());
    Ok(map.get(&format!("{}:{}", account_id, key)).cloned())
}

pub fn delete_token(account_id: &str, key: &str) -> Result<()> {
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    map.remove(&format!("{}:{}", account_id, key));
    save_to_disk(&map)
}

pub fn store_api_key(provider: &str, key: &str) -> Result<()> {
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(format!("apikey:{}", provider), key.to_string());
    save_to_disk(&map)
}

pub fn get_api_key(provider: &str) -> Result<Option<String>> {
    let map = store().lock().unwrap_or_else(|e| e.into_inner());
    Ok(map.get(&format!("apikey:{}", provider)).cloned())
}
