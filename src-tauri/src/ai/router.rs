/// Per-task provider selection — reads user config, routes to correct provider
use crate::ai::{
    ollama::OllamaProvider,
    openai_compat::OpenAICompatProvider,
    provider::{AiProvider, ChatMessage, CompletionRequest, TokenStream},
};
use crate::auth::keychain;
use crate::db::{models::AiConfig, Database};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

pub enum TaskKind {
    Triage,
    Summary,
    Draft,
    Extract,
    Embed,
}

pub struct AiRouter {
    db: Arc<Mutex<Database>>,
}

impl AiRouter {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }

    async fn get_config(&self) -> Result<AiConfig> {
        let db = self.db.lock().await;
        Ok(db.get_ai_config().unwrap_or_default())
    }

    pub async fn provider_for(&self, _task: &TaskKind) -> Result<Box<dyn AiProvider>> {
        let config = self.get_config().await?;

        if !config.enabled {
            return Err(anyhow!("AI is disabled. Configure a provider in Settings."));
        }

        match config.provider.as_str() {
            "ollama" => Ok(Box::new(OllamaProvider::new(&config.base_url))),
            "openai_compat" => {
                let api_key = keychain::get_api_key("byok")?
                    .ok_or_else(|| anyhow!("No API key configured. Add your key in Settings."))?;
                Ok(Box::new(OpenAICompatProvider::new(
                    &config.base_url,
                    &api_key,
                )))
            }
            other => Err(anyhow!("Unknown AI provider: {}", other)),
        }
    }

    pub async fn model_for(&self, task: &TaskKind) -> Result<String> {
        let config = self.get_config().await?;
        let model = match task {
            TaskKind::Triage => config.model_triage,
            TaskKind::Summary => config.model_summary,
            TaskKind::Draft => config.model_draft,
            TaskKind::Extract => config.model_extract,
            TaskKind::Embed => config.model_embed,
        };
        Ok(model)
    }

    pub async fn complete(&self, task: TaskKind, messages: Vec<ChatMessage>) -> Result<String> {
        let provider = self.provider_for(&task).await?;
        let model = self.model_for(&task).await?;
        let req = CompletionRequest {
            model,
            messages,
            temperature: Some(0.3),
            max_tokens: Some(2048),
            stream: false,
        };
        let resp = provider.complete(req).await?;
        Ok(resp.content)
    }

    pub async fn stream_complete(
        &self,
        task: TaskKind,
        messages: Vec<ChatMessage>,
    ) -> Result<TokenStream> {
        let provider = self.provider_for(&task).await?;
        let model = self.model_for(&task).await?;
        let req = CompletionRequest {
            model,
            messages,
            temperature: Some(0.5),
            max_tokens: Some(2048),
            stream: true,
        };
        provider.stream_complete(req).await
    }
}
