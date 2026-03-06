/// Bridge AI streaming tokens → Tauri events
use anyhow::Result;
use futures::StreamExt;
use tauri::{AppHandle, Emitter};

use crate::ai::provider::TokenStream;

/// Stream AI tokens to the frontend via Tauri events.
/// `event_name` is the event key the frontend listens to.
pub async fn stream_to_frontend(
    app: &AppHandle,
    mut stream: TokenStream,
    event_name: &str,
) -> Result<String> {
    let mut full_content = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(token) => {
                full_content.push_str(&token);
                // Emit each token to frontend
                let _ = app.emit(event_name, &token);
            }
            Err(e) => {
                let _ = app.emit(&format!("{}_error", event_name), e.to_string());
                return Err(e);
            }
        }
    }

    // Signal completion
    let _ = app.emit(&format!("{}_done", event_name), &full_content);
    Ok(full_content)
}
