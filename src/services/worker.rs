use std::sync::Arc;
use tracing::{error, info};
use tera::{Tera, Context};
use bon::bon;
use reqwest::Client;

use crate::services::crawler::CrawlItem;
use crate::services::documents::DocxMarkdownFetcher;
use crate::traits::markdown_fetcher::MarkdownFetcher;
use crate::services::mastodon::{MastodonPublisher, ensure_mastodon_token, load_token_from_secrets};
use crate::services::publisher::{ConsolePublisher, FilePublisher, TelegramPublisherAdapter, MastodonPublisherAdapter};
use crate::traits::publisher::Publisher;
use crate::traits::telegram_api::TelegramApi;
use crate::traits::cache_manager::CacheManager;
use crate::services::summarizer::Summarizer;
use crate::services::settings::AppConfig;
use crate::services::channels::ChannelManager;
use crate::models::channel::PublisherChannel;

/// Trim text to at most `max_chars` characters, appending an ellipsis if trimmed.
/// Uses char-aware slicing to avoid breaking UTF-8 sequences.
fn trim_with_ellipsis(text: &str, max_chars: usize) -> String {
    if max_chars == 0 { return String::new(); }
    let count = text.chars().count();
    if count <= max_chars { return text.to_string(); }
    if max_chars == 1 { return "…".to_string(); }
    let take_chars = max_chars.saturating_sub(1);
    let mut s: String = text.chars().take(take_chars).collect();
    s.push('…');
    s
}

/// Обрабатывает элементы краулинга: суммаризация, публикация
pub struct Worker {
    config: AppConfig,
    summarizer: Arc<Summarizer>,
    telegram_api: Option<Arc<dyn TelegramApi>>,
    target_chat_id: Option<i64>,
    mastodon: Option<Arc<MastodonPublisher>>,
    cache_manager: Arc<dyn CacheManager>,
    channel_manager: ChannelManager,
}

#[bon]
impl Worker {
    #[builder]
    pub async fn new(
        config: AppConfig,
        summarizer: Arc<Summarizer>,
        telegram_api: Option<Arc<dyn TelegramApi>>,
        target_chat_id: Option<i64>,
        cache_manager: Arc<dyn CacheManager>,
    ) -> std::io::Result<Self> {
        // Инициализация Mastodon
        // КРИТИЧЕСКИ ВАЖНО: Если Mastodon включен как канал публикации (enabled: true),
        // приложение требует успешной авторизации. При неудаче приложение завершается с ошибкой.
        let mastodon: Option<Arc<MastodonPublisher>> = if let Some(m) = config.mastodon.as_ref().filter(|m| m.enabled) {
            // 1) Проверяем access_token в конфигурации
            if !m.access_token.is_empty() {
                Some(Arc::new(MastodonPublisher::builder()
                    .client(Client::new())
                    .base_url(m.base_url.clone())
                    .access_token(m.access_token.clone())
                    .build()))
            } else {
                // 2) Пытаемся загрузить токен из файла secrets/mastodon.yaml
                let token_path = std::path::Path::new("./secrets/mastodon.yaml");
                match load_token_from_secrets(token_path) {
                    Ok(Some(token)) => {
                        Some(Arc::new(MastodonPublisher::builder()
                            .client(Client::new())
                            .base_url(m.base_url.clone())
                            .access_token(token)
                            .build()))
                    },
                    Ok(None) => {
                        // Проверяем, разрешен ли CLI логин
                        if m.login_cli.unwrap_or(false) {
                            // CLI логин разрешен, пытаемся авторизоваться
                            match ensure_mastodon_token(&m.base_url, token_path).await {
                                Ok(token) => Some(Arc::new(MastodonPublisher::builder()
                                    .client(Client::new())
                                    .base_url(m.base_url.clone())
                                    .access_token(token)
                                    .build())),
                                Err(e) => { 
                                    error!(error = %e, "mastodon login_cli failed"); 
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::PermissionDenied,
                                        format!("Критическая ошибка: не удалось авторизоваться в Mastodon. Mastodon включен как канал публикации, но авторизация не удалась: {}", e)
                                    ));
                                }
                            }
                        } else { 
                            // КРИТИЧЕСКАЯ ОШИБКА: Mastodon включен, но токен недоступен и CLI логин отключен
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Критическая ошибка: Mastodon включен как канал публикации, но токен доступа недоступен. Укажите access_token в конфигурации или установите login_cli: true для интерактивной авторизации."
                            ));
                        }
                    },
                    Err(e) => {
                        // Проверяем, разрешен ли CLI логин
                        if m.login_cli.unwrap_or(false) {
                            // CLI логин разрешен, пытаемся авторизоваться
                            match ensure_mastodon_token(&m.base_url, token_path).await {
                                Ok(token) => Some(Arc::new(MastodonPublisher::builder()
                                    .client(Client::new())
                                    .base_url(m.base_url.clone())
                                    .access_token(token)
                                    .build())),
                                Err(e) => { 
                                    error!(error = %e, "mastodon login_cli failed"); 
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::PermissionDenied,
                                        format!("Критическая ошибка: не удалось авторизоваться в Mastodon. Mastodon включен как канал публикации, но авторизация не удалась: {}", e)
                                    ));
                                }
                            }
                        } else { 
                            // КРИТИЧЕСКАЯ ОШИБКА: Mastodon включен, но токен недоступен и CLI логин отключен
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Критическая ошибка: Mastodon включен как канал публикации, но токен доступа недоступен. Укажите access_token в конфигурации или установите login_cli: true для интерактивной авторизации."
                            ));
                        }
                    },
                    _ => {
                        // 3) Интерактивная авторизация через CLI (если разрешена)
                        if m.login_cli.unwrap_or(false) {
                            match ensure_mastodon_token(&m.base_url, token_path).await {
                                Ok(token) => Some(Arc::new(MastodonPublisher::builder()
                                    .client(Client::new())
                                    .base_url(m.base_url.clone())
                                    .access_token(token)
                                    .build())),
                                Err(e) => { 
                                    error!(error = %e, "mastodon login_cli failed"); 
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::PermissionDenied,
                                        format!("Критическая ошибка: не удалось авторизоваться в Mastodon. Mastodon включен как канал публикации, но авторизация не удалась: {}", e)
                                    ));
                                }
                            }
                        } else { 
                            // КРИТИЧЕСКАЯ ОШИБКА: Mastodon включен, но токен недоступен и CLI логин отключен
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Критическая ошибка: Mastodon включен как канал публикации, но токен доступа недоступен. Укажите access_token в конфигурации или установите login_cli: true для интерактивной авторизации."
                            ));
                        }
                    }
                }
            }
        } else { 
            // Mastodon отключен - это нормально
            None 
        };

        let channel_manager = ChannelManager::builder().config(&config).build();

        Ok(Self {
            config,
            summarizer,
            telegram_api,
            target_chat_id,
            mastodon,
            cache_manager,
            channel_manager,
        })
    }

    /// Обрабатывает список элементов
    pub async fn process_items(&self, items: Vec<CrawlItem>) -> std::io::Result<usize> {
        let max_posts_per_run: Option<usize> = self.config
            .run
            .as_ref()
            .and_then(|r| r.max_posts_per_run);
        
        let mut published_count: usize = 0;
        
        for item in items {
            if let Some(limit) = max_posts_per_run { 
                if published_count >= limit { 
                    break; 
                } 
            }
            
            let title = if item.title.is_empty() {
                "Обновление".to_string()
            } else {
                item.title.clone()
            };
            
            let url = item.url.clone();
            let project_id = item.project_id.clone();

            // Поэтапная проверка кэша согласно схеме
            let published_names = if let Some(pid) = project_id.as_ref() {
                info!(%url, %title, project_id = %pid, "worker: processing item");
                
                // Этап 1: Проверяем наличие данных (docx/markdown)
                let (markdown_text, docx_bytes) = match self.cache_manager.has_data(pid).await {
                    Ok(true) => {
                        info!(project_id = %pid, "cache hit: using cached markdown data");
                        match self.cache_manager.load_cached_data(pid).await {
                            Ok(Some(data)) => {
                                info!(project_id = %pid, "successfully loaded cached data, len={}", data.len());
                                (data, None)
                            },
                            Ok(None) => {
                                error!(project_id = %pid, "cache inconsistency: has_data=true but load_cached_data=None");
                                (String::new(), None)
                            }
                            Err(e) => {
                                error!(project_id = %pid, error = %e, "failed to load cached data");
                                (String::new(), None)
                            }
                        }
                    }
                    Ok(false) => {
                        info!(project_id = %pid, "no cached data found; will fetch");
                        (String::new(), None)
                    }
                    Err(e) => {
                        error!(project_id = %pid, error = %e, "failed to check cached data");
                        (String::new(), None)
                    }
                };

                // Если данных нет в кэше, скачиваем их
                let (final_markdown, final_docx_bytes) = if markdown_text.is_empty() {
                    info!(project_id = %pid, "fetching markdown from source");
                    let file_id_tpl = self.config.crawler.file_id.as_ref().map(|f| f.url.clone());
                    let fetcher = DocxMarkdownFetcher::builder().maybe_file_id_url_template(file_id_tpl).build();
                    
                    match fetcher.fetch_markdown(pid).await {
                        Ok(Some((bytes, text))) => {
                            // Сохраняем данные в кэш
                            let _ = self.cache_manager.save_artifacts(
                                pid,
                                Some(&bytes),
                                &text,
                                "",
                                "",
                                &[]
                            ).await;
                            (text, Some(bytes))
                        }
                        Ok(None) => {
                            info!(project_id = %pid, "no fileId found, skipping");
                            continue;
                        }
                        Err(e) => {
                            error!(project_id = %pid, error = %e, "failed to fetch markdown");
                            continue;
                        }
                    }
                } else {
                    info!(project_id = %pid, "using cached markdown data, len={}", markdown_text.len());
                    (markdown_text, docx_bytes.clone())
                };

                // Этап 2: Проверяем наличие суммаризации
                let summary_text = match self.cache_manager.has_summary(pid).await {
                    Ok(true) => {
                        info!(project_id = %pid, "cache hit: using cached summary");
                        match self.cache_manager.load_summary(pid).await {
                            Ok(Some(summary)) => summary,
                            Ok(None) => {
                                error!(project_id = %pid, "cache inconsistency: has_summary=true but load_summary=None");
                                String::new()
                            }
                            Err(e) => {
                                error!(project_id = %pid, error = %e, "failed to load cached summary");
                                String::new()
                            }
                        }
                    }
                    Ok(false) => {
                        info!(project_id = %pid, "no cached summary found; will generate");
                        String::new()
                    }
                    Err(e) => {
                        error!(project_id = %pid, error = %e, "failed to check cached summary");
                        String::new()
                    }
                };

                // Если суммаризации нет в кэше, генерируем её
                let _final_summary = if summary_text.is_empty() {
                    info!(project_id = %pid, "generating summary");
                    let generated_summary = self.summarize_text(&title, &url, &final_markdown, &item, None).await?;
                    
                    // Сохраняем суммаризацию в кэш
                    let _ = self.cache_manager.save_artifacts(
                        pid,
                        final_docx_bytes.as_deref(),
                        &final_markdown,
                        &generated_summary,
                        "",
                        &[]
                    ).await;
                    
                    generated_summary
                } else {
                    summary_text
                };

                // Этап 3: Обрабатываем каждый канал отдельно
                let published_names = self.process_item_for_channels(pid, &title, &url, &final_markdown, &item, final_docx_bytes.as_deref()).await?;
                
                published_names
            } else {
                error!("project_id not found in url, skipping item");
                continue;
            };
            if !published_names.is_empty() { 
                published_count += 1; 
            }
        }
        
        Ok(published_count)
    }

    /// Суммаризирует текст
    async fn summarize_text(
        &self,
        title: &str,
        url: &str,
        text: &str,
        item: &CrawlItem,
        channel_limit: Option<usize>,
    ) -> std::io::Result<String> {
        // throttle LLM calls using crawler.poll_delay_secs
        let llm_delay = self.config.crawler.poll_delay_secs.unwrap_or(0);
        if llm_delay > 0 { 
            info!(
                secs = llm_delay,
                "worker: sleeping before LLM summarization call to avoid rate limiting"
            ); 
            tokio::time::sleep(std::time::Duration::from_secs(llm_delay)).await; 
        }
        
        // Используем лимит канала, если указан, иначе fallback на post_max_chars
        let model_limit = channel_limit.or_else(|| self.config.run.as_ref().and_then(|r| r.post_max_chars));
        let summarizer_arc = self.summarizer.clone();
        
        match tokio::time::timeout(
            std::time::Duration::from_secs(
                self.config.run.as_ref()
                    .and_then(|r| r.summarization_timeout_secs)
                    .unwrap_or(120)
            ),
            async move { 
                summarizer_arc.summarize_with_limit(title, text, url, Some(item.clone()), model_limit).await 
            }
        ).await {
            Ok(Ok(s)) => {
                // Раннее сохранение summary до публикации
                if let Some(pid) = item.project_id.as_ref() {
                    let _ = self.cache_manager.save_artifacts(
                        pid,
                        None,
                        text,
                        &s,
                        "",
                        &[]
                    ).await;
                }
                Ok(s)
            },
            Ok(Err(e)) => {
                error!(%e, "summarizer failed");
                Err(std::io::Error::new(std::io::ErrorKind::Other, format!("summarizer failed: {}", e)))
            }
            Err(_) => {
                error!("summarizer timeout");
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "summarizer timeout"))
            }
        }
    }


    /// Строит пост из шаблона
    fn build_post(&self, item: &CrawlItem, summary: &str) -> Result<String, std::io::Error> {
        let tpl = self.config.run.as_ref()
            .and_then(|r| r.post_template.as_ref())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "run.post_template missing"))?;
        
        let mut tera = Tera::default();
        tera.add_raw_template("post_tpl", tpl)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("invalid post_template: {}", e)))?;
        
        let mut ctx = Context::new();
        
        // Базовые поля
        ctx.insert("title", &item.title);
        ctx.insert("url", &item.url);
        ctx.insert("summary", summary);
        ctx.insert("project_id", &item.project_id);
        
        // Метаданные
        for m in &item.metadata {
            let key = m.to_string();
            let value = match m {
                crate::services::crawler::MetadataItem::Date(v) => v,
                crate::services::crawler::MetadataItem::PublishDate(v) => v,
                crate::services::crawler::MetadataItem::RegulatoryImpact(v) => v,
                crate::services::crawler::MetadataItem::RegulatoryImpactId(v) => v,
                crate::services::crawler::MetadataItem::Responsible(v) => v,
                crate::services::crawler::MetadataItem::Author(v) => v,
                crate::services::crawler::MetadataItem::Department(v) => v,
                crate::services::crawler::MetadataItem::DepartmentId(v) => v,
                crate::services::crawler::MetadataItem::Status(v) => v,
                crate::services::crawler::MetadataItem::StatusId(v) => v,
                crate::services::crawler::MetadataItem::Stage(v) => v,
                crate::services::crawler::MetadataItem::StageId(v) => v,
                crate::services::crawler::MetadataItem::Kind(v) => v,
                crate::services::crawler::MetadataItem::KindId(v) => v,
                crate::services::crawler::MetadataItem::Procedure(v) => v,
                crate::services::crawler::MetadataItem::ProcedureId(v) => v,
                crate::services::crawler::MetadataItem::ProcedureResult(v) => v,
                crate::services::crawler::MetadataItem::ProcedureResultId(v) => v,
                crate::services::crawler::MetadataItem::NextStageDuration(v) => v,
                crate::services::crawler::MetadataItem::ParallelStageStartDiscussion(v) => v,
                crate::services::crawler::MetadataItem::ParallelStageEndDiscussion(v) => v,
                crate::services::crawler::MetadataItem::StartDiscussion(v) => v,
                crate::services::crawler::MetadataItem::EndDiscussion(v) => v,
                crate::services::crawler::MetadataItem::Problem(v) => v,
                crate::services::crawler::MetadataItem::Objectives(v) => v,
                crate::services::crawler::MetadataItem::CirclePersons(v) => v,
                crate::services::crawler::MetadataItem::SocialRelations(v) => v,
                crate::services::crawler::MetadataItem::Rationale(v) => v,
                crate::services::crawler::MetadataItem::TransitionPeriod(v) => v,
                crate::services::crawler::MetadataItem::PlanDate(v) => v,
                crate::services::crawler::MetadataItem::CompliteDateAct(v) => v,
                crate::services::crawler::MetadataItem::CompliteNumberDepAct(v) => v,
                crate::services::crawler::MetadataItem::CompliteNumberRegAct(v) => v,
                crate::services::crawler::MetadataItem::ParallelStageFiles(v) => &v.join(", "),
            };
            ctx.insert(&key, value);
        }
        
        let rendered = tera.render("post_tpl", &ctx)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("post_template render failed: {}", e)))?;
        
        // Применяем жесткий лимит размера поста, если задан
        let final_post = if let Some(max_chars) = self.config.run.as_ref().and_then(|r| r.post_max_chars) {
            trim_with_ellipsis(&rendered, max_chars)
        } else {
            rendered
        };
        
        Ok(final_post)
    }

    /// Обрабатывает суммаризацию для конкретного канала
    async fn process_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        title: &str,
        url: &str,
        markdown_text: &str,
        item: &CrawlItem,
    ) -> std::io::Result<String> {
        // Проверяем, есть ли уже суммаризация для этого канала
        match self.cache_manager.has_channel_summary(project_id, channel).await {
            Ok(true) => {
                info!(project_id = %project_id, channel = %channel, "cache hit: using cached channel summary");
                match self.cache_manager.load_channel_summary(project_id, channel).await {
                    Ok(Some(summary)) => {
                        info!(project_id = %project_id, channel = %channel, "successfully loaded cached channel summary, len={}", summary.len());
                        return Ok(summary.into_inner());
                    },
                    Ok(None) => {
                        error!(project_id = %project_id, channel = %channel, "cache inconsistency: has_channel_summary=true but load_channel_summary=None");
                    }
                    Err(e) => {
                        error!(project_id = %project_id, channel = %channel, error = %e, "failed to load cached channel summary");
                    }
                }
            }
            Ok(false) => {
                info!(project_id = %project_id, channel = %channel, "no cached channel summary found; will generate");
            }
            Err(e) => {
                error!(project_id = %project_id, channel = %channel, error = %e, "failed to check cached channel summary");
            }
        }

        // Получаем лимит символов для канала
        let channel_limit = self.channel_manager.get_channel_limit(channel)
            .unwrap_or(300); // fallback лимит

        info!(
            project_id = %project_id,
            channel = %channel,
            limit = channel_limit,
            "generating channel-specific summary"
        );

        // Генерируем суммаризацию для конкретного канала
        let summary = self.summarize_text(title, url, markdown_text, item, Some(channel_limit)).await?;

        // Сохраняем суммаризацию в кэш для этого канала
        if let Err(e) = self.cache_manager.save_channel_summary(project_id, channel, &summary).await {
            error!(project_id = %project_id, channel = %channel, error = %e, "failed to save channel summary to cache");
        }

        Ok(summary)
    }

    /// Обрабатывает пост для конкретного канала
    async fn process_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        _title: &str,
        _url: &str,
        summary: &str,
        item: &CrawlItem,
    ) -> std::io::Result<String> {
        // Проверяем, есть ли уже пост для этого канала
        match self.cache_manager.has_channel_post(project_id, channel).await {
            Ok(true) => {
                info!(project_id = %project_id, channel = %channel, "cache hit: using cached channel post");
                match self.cache_manager.load_channel_post(project_id, channel).await {
                    Ok(Some(post)) => {
                        info!(project_id = %project_id, channel = %channel, "successfully loaded cached channel post, len={}", post.len());
                        return Ok(post.into_inner());
                    },
                    Ok(None) => {
                        error!(project_id = %project_id, channel = %channel, "cache inconsistency: has_channel_post=true but load_channel_post=None");
                    }
                    Err(e) => {
                        error!(project_id = %project_id, channel = %channel, error = %e, "failed to load cached channel post");
                    }
                }
            }
            Ok(false) => {
                info!(project_id = %project_id, channel = %channel, "no cached channel post found; will generate");
            }
            Err(e) => {
                error!(project_id = %project_id, channel = %channel, error = %e, "failed to check cached channel post");
            }
        }

        // Генерируем пост для конкретного канала
        let post = self.build_post(item, summary)?;

        // Сохраняем пост в кэш для этого канала
        if let Err(e) = self.cache_manager.save_channel_post(project_id, channel, &post).await {
            error!(project_id = %project_id, channel = %channel, error = %e, "failed to save channel post to cache");
        }

        Ok(post)
    }

    /// Обрабатывает элемент для всех включенных каналов с индивидуальными суммаризациями
    async fn process_item_for_channels(
        &self,
        project_id: &str,
        title: &str,
        url: &str,
        markdown_text: &str,
        item: &CrawlItem,
        _docx_bytes: Option<&[u8]>,
    ) -> std::io::Result<Vec<String>> {
        let mut published_channels = Vec::new();
        
        // Получаем список всех включенных каналов
        let enabled_channels = self.channel_manager.get_enabled_channels();
        
        for channel_config in enabled_channels {
            let channel = channel_config.channel;
            let channel_name = channel.as_str();
            
            // Проверяем, не опубликован ли уже в этом канале
            if self.cache_manager.is_published_in_channel(project_id, channel).await.unwrap_or(false) {
                info!(project_id = %project_id, channel = %channel_name, "skip republish: channel already published");
                continue;
            }
            
            // Генерируем суммаризацию для этого канала
            let channel_summary = self.process_channel_summary(
                project_id,
                channel,
                title,
                url,
                markdown_text,
                item,
            ).await?;
            
            // Генерируем пост для этого канала
            let channel_post = self.process_channel_post(
                project_id,
                channel,
                title,
                url,
                &channel_summary,
                item,
            ).await?;
            
            // Публикуем в канале
            match self.publish_to_channel(channel, &channel_post, &item).await {
                Ok(success) => {
                    if success {
                        published_channels.push(channel_name.to_string());
                        info!(project_id = %project_id, channel = %channel_name, "successfully published to channel");
                    } else {
                        info!(project_id = %project_id, channel = %channel_name, "publication to channel skipped");
                    }
                }
                Err(e) => {
                    error!(project_id = %project_id, channel = %channel_name, error = %e, "failed to publish to channel");
                }
            }
        }
        
        // Обновляем список опубликованных каналов в кэше
        if !published_channels.is_empty() {
            // Конвертируем строки обратно в PublisherChannel для кэша
            let channels: Result<Vec<PublisherChannel>, _> = published_channels.iter()
                .map(|s| PublisherChannel::from_str(s))
                .collect();
            
            if let Ok(channels) = channels {
                if let Err(e) = self.cache_manager.add_published_channels(project_id, &channels).await {
                    error!(project_id = %project_id, error = %e, "failed to update published channels in cache");
                }
            }
        }
        
        Ok(published_channels)
    }

    /// Публикует пост в конкретном канале
    async fn publish_to_channel(
        &self,
        channel: PublisherChannel,
        post_text: &str,
        item: &CrawlItem,
    ) -> std::io::Result<bool> {
        match channel {
            PublisherChannel::Telegram => {
                if let (Some(api), Some(chat_id)) = (&self.telegram_api, &self.target_chat_id) {
                    let publisher = TelegramPublisherAdapter { 
                        api: api.clone(), 
                        chat_id: *chat_id,
                        max_chars: self.channel_manager.get_channel_limit(PublisherChannel::Telegram)
                    };
                    match publisher.publish(&item.title, &item.url, post_text).await {
                        Ok(_) => Ok(true),
                        Err(e) => {
                            error!(error = %e, "telegram publish failed");
                            Ok(false)
                        }
                    }
                } else {
                    info!("telegram: disabled or not configured");
                    Ok(false)
                }
            }
            PublisherChannel::Mastodon => {
                if let Some(mastodon) = &self.mastodon {
                    let publisher = MastodonPublisherAdapter { 
                        client: mastodon.clone(),
                        visibility: self.config.mastodon.as_ref().and_then(|m| m.visibility.clone()),
                        language: self.config.mastodon.as_ref().and_then(|m| m.language.clone()),
                        spoiler_text: self.config.mastodon.as_ref().and_then(|m| m.spoiler_text.clone()),
                        sensitive: self.config.mastodon.as_ref().and_then(|m| m.sensitive).unwrap_or(false),
                        max_chars: self.channel_manager.get_channel_limit(PublisherChannel::Mastodon)
                    };
                    match publisher.publish(&item.title, &item.url, post_text).await {
                        Ok(_) => Ok(true),
                        Err(e) => {
                            error!(error = %e, "mastodon publish failed");
                            Ok(false)
                        }
                    }
                } else {
                    info!("mastodon: disabled or not configured");
                    Ok(false)
                }
            }
            PublisherChannel::Console => {
                let publisher = ConsolePublisher { max_chars: self.channel_manager.get_channel_limit(PublisherChannel::Console) };
                match publisher.publish(&item.title, &item.url, post_text).await {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        error!(error = %e, "console publish failed");
                        Ok(false)
                    }
                }
            }
            PublisherChannel::File => {
                let file_path = self.config.output.as_ref()
                    .and_then(|o| o.file_path.clone())
                    .unwrap_or_else(|| "./post.txt".to_string());
                let publisher = FilePublisher { 
                    path: file_path,
                    max_chars: self.channel_manager.get_channel_limit(PublisherChannel::File),
                    append: self.config.output.as_ref().and_then(|o| o.file_append).unwrap_or(false)
                };
                match publisher.publish(&item.title, &item.url, post_text).await {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        error!(error = %e, "file publish failed");
                        Ok(false)
                    }
                }
            }
        }
    }
}
