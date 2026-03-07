/// AiProvider trait — provider-agnostic AI interface
use anyhow::Result;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

pub type TokenStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;
    async fn stream_complete(&self, req: CompletionRequest) -> Result<TokenStream>;
    fn supports_tools(&self) -> bool;
    fn name(&self) -> &str;
}

/// Truncate thread content to fit within token budget (rough estimate: 4 chars ≈ 1 token)
pub fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens * 4;
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!("{}...[truncated]", &text[..max_chars])
    }
}

/// Build thread context string from messages
pub fn build_thread_context(
    messages: &[crate::db::models::Message],
    max_tokens: usize,
    privacy_mode: bool,
) -> String {
    let mut parts = Vec::new();
    let per_msg_budget = max_tokens / messages.len().max(1);

    for msg in messages {
        let from = if privacy_mode {
            "[sender]".to_string()
        } else {
            msg.from
                .first()
                .map(|a| a.name.as_deref().unwrap_or(&a.email).to_string())
                .unwrap_or_default()
        };

        let date = msg
            .date
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();

        let body = msg
            .body_text
            .as_deref()
            .or(msg.body_html.as_deref())
            .unwrap_or("[no body]");

        let body = truncate_to_tokens(body, per_msg_budget);
        parts.push(format!(
            "--- From: {} | Date: {} ---\n{}\n",
            from, date, body
        ));
    }

    parts.join("\n")
}
