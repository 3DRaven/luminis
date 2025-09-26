use std::sync::Arc;
use std::time::Duration;

use crate::models::types::CrawlItem;
use crate::models::config::AppConfig;
use crate::traits::chat_api::ChatApi;
use backon::{ExponentialBuilder, Retryable};
use bon::Builder;
use tera::{Context, Tera};
use tracing::{debug, info, warn};

/// Service that wraps `ChatApi` and generates concise Telegram-ready posts
/// from raw website content.
#[derive(Builder)]
pub struct Summarizer {
    chat_api: Arc<dyn ChatApi>,
    hard_max_chars: usize,
    sample_percent: f32,
    template: Option<String>,
    preview_chars: Option<usize>,
    max_retry_attempts: u64,
    retry_delay_secs: u64,
}

impl Summarizer {
    pub fn with_config(mut self, cfg: &AppConfig) -> Self {
        if let Some(run) = cfg.run.as_ref() {
            if let Some(p) = run.input_sample_percent {
                // ожидается доля [0.0..1.0], например 0.05 = 5%
                self.sample_percent = p.clamp(0.001, 1.0);
            }
        }
        if let Some(run) = cfg.run.as_ref() {
            if let Some(tpl) = run.prompt_template.clone() {
                self.template = Some(tpl);
            }
        }
        // Настройка длины превью для логов промпта
        self.preview_chars = cfg.llm.log_prompt_preview_chars;
        // Настройка параметров retry
        self.max_retry_attempts = cfg.llm.max_retry_attempts.unwrap_or(3);
        self.retry_delay_secs = cfg.llm.retry_delay_secs.unwrap_or(2);
        self
    }

    /// Builds a prompt by rendering a Tera template from config.
    fn build_prompt(
        &self,
        title: &str,
        body_text: &str,
        source_url: &str,
        meta: Option<&CrawlItem>,
        model_limit: Option<usize>,
    ) -> String {
        // limit: prefer per-call model_limit, else fallback to hard_max_chars as a coarse hint
        let limit = model_limit.unwrap_or(self.hard_max_chars);
        // take leading slice of the text by sample_percent
        // символобезопасное усечение (по char), чтобы не резать UTF-8 на байтах
        let total_chars = body_text.chars().count();
        let take_chars = (((total_chars as f32) * self.sample_percent).max(1.0)) as usize;
        let take_chars = take_chars.min(total_chars);
        let sampled: String = body_text.chars().take(take_chars).collect();

        if let Some(tpl) = &self.template {
            let mut tera = Tera::default();
            // Register ad-hoc template name
            let template_name = "summarizer_prompt";
            if let Err(e) = tera.add_raw_template(template_name, tpl) {
                warn!("tera add_raw_template failed: {}", e);
            }
            let mut ctx = Context::new();
            ctx.insert("limit", &limit);
            ctx.insert("title", &title);
            ctx.insert("body", &sampled);
            ctx.insert("url", &source_url);
            if let Some(m) = meta {
                // Insert project_id and all metadata items into template context
                ctx.insert("project_id", &m.project_id);
                for it in &m.metadata {
                    let key = it.to_string();
                    let value = match it {
                        crate::models::types::MetadataItem::Date(v) => v,
                        crate::models::types::MetadataItem::PublishDate(v) => v,
                        crate::models::types::MetadataItem::RegulatoryImpact(v) => v,
                        crate::models::types::MetadataItem::RegulatoryImpactId(v) => v,
                        crate::models::types::MetadataItem::Responsible(v) => v,
                        crate::models::types::MetadataItem::Author(v) => v,
                        crate::models::types::MetadataItem::Department(v) => v,
                        crate::models::types::MetadataItem::DepartmentId(v) => v,
                        crate::models::types::MetadataItem::Status(v) => v,
                        crate::models::types::MetadataItem::StatusId(v) => v,
                        crate::models::types::MetadataItem::Stage(v) => v,
                        crate::models::types::MetadataItem::StageId(v) => v,
                        crate::models::types::MetadataItem::Kind(v) => v,
                        crate::models::types::MetadataItem::KindId(v) => v,
                        crate::models::types::MetadataItem::Procedure(v) => v,
                        crate::models::types::MetadataItem::ProcedureId(v) => v,
                        crate::models::types::MetadataItem::ProcedureResult(v) => v,
                        crate::models::types::MetadataItem::ProcedureResultId(v) => v,
                        crate::models::types::MetadataItem::NextStageDuration(v) => v,
                        crate::models::types::MetadataItem::ParallelStageStartDiscussion(v) => v,
                        crate::models::types::MetadataItem::ParallelStageEndDiscussion(v) => v,
                        crate::models::types::MetadataItem::StartDiscussion(v) => v,
                        crate::models::types::MetadataItem::EndDiscussion(v) => v,
                        crate::models::types::MetadataItem::Problem(v) => v,
                        crate::models::types::MetadataItem::Objectives(v) => v,
                        crate::models::types::MetadataItem::CirclePersons(v) => v,
                        crate::models::types::MetadataItem::SocialRelations(v) => v,
                        crate::models::types::MetadataItem::Rationale(v) => v,
                        crate::models::types::MetadataItem::TransitionPeriod(v) => v,
                        crate::models::types::MetadataItem::PlanDate(v) => v,
                        crate::models::types::MetadataItem::CompliteDateAct(v) => v,
                        crate::models::types::MetadataItem::CompliteNumberDepAct(v) => v,
                        crate::models::types::MetadataItem::CompliteNumberRegAct(v) => v,
                        crate::models::types::MetadataItem::ParallelStageFiles(v) => &v.join(", "),
                    };
                    ctx.insert(&key, value);
                }
            }
            match tera.render(template_name, &ctx) {
                Ok(s) => {
                    let preview_len = self.preview_chars.unwrap_or(200);
                    let preview: String = s.chars().take(preview_len).collect();
                    info!(limit = limit, prompt_len = s.len(), prompt_preview = %preview, "summarize: prompt rendered");
                    s
                }
                Err(e) => {
                    warn!("tera render failed: {}", e);
                    sampled
                }
            }
        } else {
            sampled
        }
    }

    /// Выполняет вызов AI API с retry логикой для обработки ошибок перегрузки
    async fn call_chat_api_with_retry(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let fetch_data = || async {
            self.chat_api.call_chat_api(prompt).await
        };

        // Настраиваем retry стратегию
        let mut builder = ExponentialBuilder::default();
        if self.max_retry_attempts > 0 {
            builder = builder.with_max_times(self.max_retry_attempts as usize);
        }
        builder = builder.with_min_delay(Duration::from_secs(self.retry_delay_secs));

        fetch_data
            .retry(builder)
            .sleep(tokio::time::sleep)
            .when(|e: &Box<dyn std::error::Error + Send + Sync>| {
                let error_str = e.to_string();
                // Повторяем попытку при ошибках перегрузки сервера
                error_str.contains("503") || 
                error_str.contains("overloaded") || 
                error_str.contains("UNAVAILABLE") ||
                error_str.contains("429") ||
                error_str.contains("rate limit") ||
                error_str.contains("Network error")
            })
            .notify(|err: &Box<dyn std::error::Error + Send + Sync>, dur: Duration| {
                info!(
                    "Retrying AI API call after {:?} due to error: {}",
                    dur, err
                );
            })
            .await
    }

    pub async fn summarize(
        &self,
        title: &str,
        body_text: &str,
        source_url: &str,
        meta: Option<CrawlItem>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            title_len = title.len(),
            body_len = body_text.len(),
            "summarize: start"
        );
        // fallback to none: caller may prefer dedicated API using run.model_max_chars
        let prompt = self.build_prompt(title, body_text, source_url, meta.as_ref(), None);
        debug!(prompt_len = prompt.len(), "summarize: prompt built");
        info!("summarize: calling chat api");
        let text = self.call_chat_api_with_retry(&prompt).await?;
        info!(generated_len = text.len(), "summarize: chat api returned");
        info!(final_len = text.len(), "summarize: done");
        Ok(text)
    }

    pub async fn summarize_with_limit(
        &self,
        title: &str,
        body_text: &str,
        source_url: &str,
        meta: Option<CrawlItem>,
        model_limit: Option<usize>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(title_len = title.len(), body_len = body_text.len(), limit = ?model_limit, "summarize: start with limit");
        let prompt = self.build_prompt(title, body_text, source_url, meta.as_ref(), model_limit);
        debug!(prompt_len = prompt.len(), "summarize: prompt built");
        info!("summarize: calling chat api");
        let text = self.call_chat_api_with_retry(&prompt).await?;
        info!(generated_len = text.len(), "summarize: chat api returned");
        info!(final_len = text.len(), "summarize: done");
        Ok(text)
    }
}
