use crate::services::settings::LlmConfig;
use crate::traits::chat_api::ChatApi;
use async_trait::async_trait;
// tracing is available if needed

use ai_lib::ConnectionOptions;
use ai_lib::prelude::*;
use bon::Builder;
use std::str::FromStr;
use strum_macros::EnumString;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Clone, EnumString)]
#[strum(ascii_case_insensitive)]
enum ProviderName {
    Groq,
    XaiGrok,
    Ollama,
    DeepSeek,
    Anthropic,
    AzureOpenAI,
    HuggingFace,
    TogetherAI,
    OpenRouter,
    Replicate,
    BaiduWenxin,
    TencentHunyuan,
    IflytekSpark,
    Moonshot,
    ZhipuAI,
    MiniMax,
    OpenAI,
    Qwen,
    Gemini,
    Mistral,
    Cohere,
    Perplexity,
    AI21,
}

fn map_provider(p: ProviderName) -> Provider {
    match p {
        ProviderName::Groq => Provider::Groq,
        ProviderName::XaiGrok => Provider::XaiGrok,
        ProviderName::Ollama => Provider::Ollama,
        ProviderName::DeepSeek => Provider::DeepSeek,
        ProviderName::Anthropic => Provider::Anthropic,
        ProviderName::AzureOpenAI => Provider::AzureOpenAI,
        ProviderName::HuggingFace => Provider::HuggingFace,
        ProviderName::TogetherAI => Provider::TogetherAI,
        ProviderName::OpenRouter => Provider::OpenRouter,
        ProviderName::Replicate => Provider::Replicate,
        ProviderName::BaiduWenxin => Provider::BaiduWenxin,
        ProviderName::TencentHunyuan => Provider::TencentHunyuan,
        ProviderName::IflytekSpark => Provider::IflytekSpark,
        ProviderName::Moonshot => Provider::Moonshot,
        ProviderName::ZhipuAI => Provider::ZhipuAI,
        ProviderName::MiniMax => Provider::MiniMax,
        ProviderName::OpenAI => Provider::OpenAI,
        ProviderName::Qwen => Provider::Qwen,
        ProviderName::Gemini => Provider::Gemini,
        ProviderName::Mistral => Provider::Mistral,
        ProviderName::Cohere => Provider::Cohere,
        ProviderName::Perplexity => Provider::Perplexity,
        ProviderName::AI21 => Provider::AI21,
    }
}

/// LocalChatApi uses a cloud provider via ai-lib.
struct Engine {
    cloud: AiClient,
}

#[derive(Builder)]
pub struct LocalChatApi {
    pub model: String,
    pub model_path: Option<String>,
    pub tokenizer_path: Option<String>,
    engine: Mutex<Option<Engine>>,
}

impl LocalChatApi {
    pub fn from_config(llm: &LlmConfig) -> Self {
        llm_defaults::init(llm);
        Self {
            model: llm.model.clone().unwrap_or_else(|| "".to_string()),
            model_path: llm.model_path.clone(),
            tokenizer_path: llm.tokenizer_path.clone(),
            engine: Mutex::new(None),
        }
    }

    async fn ensure_engine(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut guard = self.engine.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        // Configure ai-lib client from config/env
        let provider = llm_defaults::provider().unwrap_or_else(|| "Groq".to_string());
        let prov = ProviderName::from_str(&provider)
            .map(map_provider)
            .unwrap_or(Provider::Groq);

        info!(
            provider = %provider,
            base_url = %llm_defaults::base_url().as_deref().unwrap_or("None"),
            proxy = %llm_defaults::proxy().as_deref().unwrap_or("None"),
            timeout = %llm_defaults::timeout().map_or("None".to_string(), |t| t.to_string()),
        );

        let client = AiClient::with_options(
            prov,
            ConnectionOptions {
                base_url: llm_defaults::base_url(),
                proxy: llm_defaults::proxy(),
                api_key: std::env::var(format!("{}_API_KEY", provider.to_uppercase()))
                    .ok()
                    .or_else(|| llm_defaults::api_key()),
                timeout: llm_defaults::timeout().map(std::time::Duration::from_secs),
                disable_proxy: false,
            },
        )?;
        *guard = Some(Engine { cloud: client });
        Ok(())
    }
}

#[async_trait]
impl ChatApi for LocalChatApi {
    async fn call_chat_api(
        &self,
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.ensure_engine().await?;
        let mut guard = self.engine.lock().await;
        let engine = guard.as_mut().expect("engine initialized");
        let client = &engine.cloud;
        // Log request details (without leaking entire prompt)
        let model_name = if self.model.trim().is_empty() {
            client.default_chat_model().to_string()
        } else {
            self.model.clone()
        };
        let preview_len: usize = llm_defaults::log_prompt_preview_chars().unwrap_or(200);
        let prompt_preview: String = prompt.chars().take(preview_len).collect();
        info!(
            model = %model_name,
            prompt_len = prompt.len(),
            prompt_preview = %prompt_preview,
            "ai_lib: chat request"
        );

        let req = ChatCompletionRequest::new(
            if self.model.trim().is_empty() {
                client.default_chat_model().to_string()
            } else {
                self.model.clone()
            },
            vec![Message {
                role: Role::User,
                content: Content::new_text(prompt.to_string()),
                function_call: None,
            }],
        );
        let resp = client.chat_completion(req).await?;
        let text = resp.choices[0].message.content.as_text();
        let preview_len: usize = llm_defaults::log_prompt_preview_chars().unwrap_or(200);
        let response_preview: String = text.chars().take(preview_len).collect();
        info!(
            model = %model_name,
            response_len = text.len(),
            response_preview = %response_preview,
            "ai_lib: chat response"
        );
        Ok(text)
    }
}

mod llm_defaults {
    use super::LlmConfig;
    use once_cell::sync::OnceCell;

    static CFG: OnceCell<LlmConfig> = OnceCell::new();

    pub fn init(cfg: &LlmConfig) {
        let _ = CFG.set(cfg.clone());
    }
    pub fn provider() -> Option<String> {
        CFG.get().and_then(|c| c.provider.clone())
    }
    pub fn base_url() -> Option<String> {
        CFG.get().and_then(|c| c.base_url.clone())
    }
    pub fn proxy() -> Option<String> {
        CFG.get().and_then(|c| c.proxy.clone())
    }
    pub fn timeout() -> Option<u64> {
        CFG.get().and_then(|c| c.request_timeout_secs)
    }
    pub fn api_key() -> Option<String> {
        CFG.get().and_then(|c| c.api_key.clone())
    }
    pub fn log_prompt_preview_chars() -> Option<usize> {
        CFG.get().and_then(|c| c.log_prompt_preview_chars)
    }
}
