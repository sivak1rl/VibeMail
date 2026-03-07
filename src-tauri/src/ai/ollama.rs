/// Ollama local provider — calls localhost:11434/api/chat
use crate::ai::provider::*;
use anyhow::{anyhow, Result};
use async_stream::stream;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct OllamaProvider {
    client: Client,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: &'a [OllamaMessage],
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
    num_predict: i32,
}

#[derive(Serialize, Deserialize, Clone)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: Option<OllamaMessage>,
    done: Option<bool>,
    eval_count: Option<u32>,
    prompt_eval_count: Option<u32>,
}

#[async_trait::async_trait]
impl AiProvider for OllamaProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse> {
        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let body = OllamaRequest {
            model: &req.model,
            messages: &messages,
            stream: false,
            options: OllamaOptions {
                temperature: req.temperature.unwrap_or(0.3),
                num_predict: req.max_tokens.unwrap_or(2048) as i32,
            },
        };

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let data: OllamaResponse = resp.json().await?;
        let content = data.message.map(|m| m.content).unwrap_or_default();

        Ok(CompletionResponse {
            content,
            input_tokens: data.prompt_eval_count,
            output_tokens: data.eval_count,
        })
    }

    async fn stream_complete(&self, req: CompletionRequest) -> Result<TokenStream> {
        let messages: Vec<OllamaMessage> = req
            .messages
            .iter()
            .map(|m| OllamaMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let body = OllamaRequest {
            model: &req.model,
            messages: &messages,
            stream: true,
            options: OllamaOptions {
                temperature: req.temperature.unwrap_or(0.3),
                num_predict: req.max_tokens.unwrap_or(2048) as i32,
            },
        };

        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
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
                                if line.is_empty() { continue; }
                                if let Ok(data) = serde_json::from_str::<OllamaResponse>(line) {
                                    if let Some(msg) = data.message {
                                        if !msg.content.is_empty() {
                                            yield Ok(msg.content);
                                        }
                                    }
                                    if data.done == Some(true) {
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
        // Ollama supports tool-calling for compatible models (llama3.1+)
        true
    }

    fn name(&self) -> &str {
        "ollama"
    }
}
