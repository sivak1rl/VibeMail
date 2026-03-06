/// OpenAI-compatible provider (Claude, OpenAI, Mistral, Groq, LM Studio, etc.)
use crate::ai::provider::*;
use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct OpenAICompatProvider {
    client: Client,
    base_url: String,
    api_key: String,
}

impl OpenAICompatProvider {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }
}

#[derive(Serialize)]
struct OAIRequest<'a> {
    model: &'a str,
    messages: Vec<OAIMessage>,
    temperature: f64,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct OAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OAIResponse {
    choices: Vec<OAIChoice>,
    usage: Option<OAIUsage>,
}

#[derive(Deserialize)]
struct OAIChoice {
    message: Option<OAIMessage>,
    delta: Option<OAIMessage>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OAIUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct OAIStreamChunk {
    choices: Vec<OAIChoice>,
}

#[async_trait::async_trait]
impl AiProvider for OpenAICompatProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let messages: Vec<OAIMessage> = req
            .messages
            .iter()
            .map(|m| OAIMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let body = OAIRequest {
            model: &req.model,
            messages,
            temperature: req.temperature.unwrap_or(0.3),
            max_tokens: req.max_tokens.unwrap_or(2048),
            stream: false,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let data: OAIResponse = resp.json().await?;
        let content = data
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .unwrap_or_default();

        Ok(CompletionResponse {
            content,
            input_tokens: data.usage.as_ref().and_then(|u| u.prompt_tokens),
            output_tokens: data.usage.as_ref().and_then(|u| u.completion_tokens),
        })
    }

    async fn stream_complete(&self, req: CompletionRequest) -> Result<TokenStream> {
        let messages: Vec<OAIMessage> = req
            .messages
            .iter()
            .map(|m| OAIMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let body = OAIRequest {
            model: &req.model,
            messages,
            temperature: req.temperature.unwrap_or(0.3),
            max_tokens: req.max_tokens.unwrap_or(2048),
            stream: true,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let byte_stream = resp.bytes_stream();

        let token_stream = stream! {
            let mut lines = Box::pin(byte_stream);
            while let Some(chunk) = lines.next().await {
                match chunk {
                    Ok(bytes) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            for line in text.lines() {
                                let line = line.trim();
                                if line.is_empty() || line == "data: [DONE]" { continue; }
                                let json_str = line.strip_prefix("data: ").unwrap_or(line);
                                if let Ok(chunk) = serde_json::from_str::<OAIStreamChunk>(json_str) {
                                    if let Some(delta) = chunk.choices.first().and_then(|c| c.delta.as_ref()) {
                                        if !delta.content.is_empty() {
                                            yield Ok(delta.content.clone());
                                        }
                                    }
                                    if chunk.choices.first().and_then(|c| c.finish_reason.as_deref()) == Some("stop") {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(anyhow!("Stream error: {}", e));
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(token_stream))
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "openai_compat"
    }
}
