pub mod services;

use std::sync::Arc;
use tracing::{error, info};

use crate::services::chat_api::ChatApi;
use crate::services::chat_api_local::LocalChatApi;
use crate::services::crawler::{RssScanner, NpaListScanner, CrawlItem};
use crate::services::documents::{DocumentFetcher, save_cache_artifacts, load_cache_metadata, load_cached_summary, add_published_channels};
use crate::services::mastodon::{MastodonPublisher, ensure_mastodon_token, load_token_from_secrets};
use crate::services::publisher::{Publisher, ConsolePublisher, FilePublisher, TelegramPublisherAdapter, MastodonPublisherAdapter};
use crate::services::settings::{AppConfig, load_config};
use crate::services::summarizer::Summarizer;
use crate::services::telegram_api::TelegramApi;
use crate::services::telegram_api_impl::RealTelegramApi;
use tera::{Tera, Context};

/// High-level entrypoint: load config, init logging, run worker
pub async fn run_with_config_path(path: &str) -> std::io::Result<()> {
    // Load YAML config
    let cfg: AppConfig = load_config(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to load {}: {}", path, e)))?;

    // Initialize structured logging (default to info if RUST_LOG not set)
    let log_spec = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_spec))
        .with_target(false)
        .compact()
        .try_init();

    run_worker(cfg).await
}

/// Worker runner: initializes services and executes the crawl/summarize/publish loop
pub async fn run_worker(cfg: AppConfig) -> std::io::Result<()> {
    info!("worker mode starting");

    // Initialize shared services from config
    let chat_api: Arc<dyn ChatApi> = Arc::new(LocalChatApi::from_config(&cfg.llm));
    let summarizer = Arc::new(Summarizer::new(Arc::clone(&chat_api), 600).with_config(&cfg));

    let (telegram_api, target_chat_id) =
        if let Some(tg) = cfg.telegram.clone().filter(|t| t.enabled) {
            let api: Arc<dyn TelegramApi> =
                Arc::new(RealTelegramApi::new(tg.api_base_url, tg.bot_token));
            (Some(api), Some(tg.target_chat_id))
        } else {
            (None, None)
        };

    // Build scanners
    let req_timeout = std::time::Duration::from_secs(cfg.crawler.request_timeout_secs.unwrap_or(30));
    let rss_scanner = RssScanner::new(req_timeout).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let npa_scanner = NpaListScanner::new(req_timeout).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    // Ensure post template is provided
    if cfg.run.as_ref().and_then(|r| r.post_template.as_ref()).is_none() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "run.post_template is required in config (no fallback post formatting)"));
    }

    // helper to build final post strictly via template
    let build_post = |cfg: &AppConfig, item: &CrawlItem, summary: &str| -> Result<String, std::io::Error> {
        let tpl = cfg.run.as_ref().and_then(|r| r.post_template.as_ref()).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "run.post_template missing"))?;
        let mut tera = Tera::default();
        tera.add_raw_template("post_tpl", tpl).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("invalid post_template: {}", e)))?;
        let mut ctx = Context::new();
        // Base
        ctx.insert("title", &item.title);
        ctx.insert("url", &item.url);
        ctx.insert("summary", summary);
        // Metadata
        ctx.insert("project_id", &item.project_id);
        ctx.insert("date", &item.date);
        ctx.insert("publish_date", &item.publish_date);
        ctx.insert("status", &item.status);
        ctx.insert("status_id", &item.status_id);
        ctx.insert("stage", &item.stage);
        ctx.insert("stage_id", &item.stage_id);
        ctx.insert("regulatory_impact", &item.regulatory_impact);
        ctx.insert("regulatory_impact_id", &item.regulatory_impact_id);
        ctx.insert("kind", &item.kind);
        ctx.insert("kind_id", &item.kind_id);
        ctx.insert("department", &item.department);
        ctx.insert("department_id", &item.department_id);
        ctx.insert("responsible", &item.responsible);
        ctx.insert("procedure", &item.procedure);
        ctx.insert("procedure_id", &item.procedure_id);
        ctx.insert("procedure_result", &item.procedure_result);
        ctx.insert("procedure_result_id", &item.procedure_result_id);
        ctx.insert("next_stage_duration", &item.next_stage_duration);
        ctx.insert("parallel_stage_start_discussion", &item.parallel_stage_start_discussion);
        ctx.insert("parallel_stage_end_discussion", &item.parallel_stage_end_discussion);
        ctx.insert("start_discussion", &item.start_discussion);
        ctx.insert("end_discussion", &item.end_discussion);
        ctx.insert("problem", &item.problem);
        ctx.insert("objectives", &item.objectives);
        ctx.insert("circle_persons", &item.circle_persons);
        ctx.insert("social_relations", &item.social_relations);
        ctx.insert("rationale", &item.rationale);
        ctx.insert("transition_period", &item.transition_period);
        ctx.insert("plan_date", &item.plan_date);
        ctx.insert("complite_date_act", &item.complite_date_act);
        ctx.insert("complite_number_dep_act", &item.complite_number_dep_act);
        ctx.insert("complite_number_reg_act", &item.complite_number_reg_act);
        ctx.insert("parallel_stage_files", &item.parallel_stage_files);
        let rendered = tera.render("post_tpl", &ctx).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("post_template render failed: {}", e)))?;
        Ok(rendered)
    };

    let interval_secs: u64 = cfg.crawler.interval_seconds;

    // Optional Mastodon (support login_cli)
    let mastodon: Option<Arc<MastodonPublisher>> = if let Some(m) = cfg.mastodon.as_ref().filter(|m| m.enabled) {
        // 1) access_token in config
        if !m.access_token.is_empty() {
            Some(Arc::new(MastodonPublisher::new(m.base_url.clone(), m.access_token.clone())))
        } else {
            // 2) secrets file
            let token_path = std::path::Path::new("./secrets/mastodon.yaml");
            match load_token_from_secrets(token_path) {
                Ok(Some(token)) => Some(Arc::new(MastodonPublisher::new(m.base_url.clone(), token))),
                _ => {
                    // 3) CLI login if allowed
                    if m.login_cli.unwrap_or(false) {
                        match ensure_mastodon_token(&m.base_url, token_path).await {
                            Ok(token) => Some(Arc::new(MastodonPublisher::new(m.base_url.clone(), token))),
                            Err(e) => { error!(error = %e, "mastodon login_cli failed"); None }
                        }
                    } else { None }
                }
            }
        }
    } else { None };

    info!("worker started; interval_secs={}", interval_secs);
    let max_posts_per_run: Option<usize> = cfg
        .run
        .as_ref()
        .and_then(|r| r.max_posts_per_run);
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    loop {
        if max_posts_per_run.is_none() {
            ticker.tick().await;
        }
        let items: Vec<CrawlItem> = {
            let mut collected: Vec<CrawlItem> = Vec::new();
            let use_npa = cfg.crawler.npalist.as_ref().map(|n| n.enabled.unwrap_or(true)).unwrap_or(false);
            let use_rss = cfg.crawler.rss.as_ref().map(|r| r.enabled.unwrap_or(true)).unwrap_or(false);
            // Try NPA first if enabled
            if use_npa {
                if let Some(npa) = cfg.crawler.npalist.as_ref() {
                    let npa_re = npa.regex.as_ref().and_then(|s| regex::Regex::new(s).ok());
                    match npa_scanner.fetch(&npa.url, npa.limit, npa_re.as_ref()).await {
                        Ok(mut v) => { if !v.is_empty() { collected.append(&mut v); } }
                        Err(e) => { error!("npalist fetch failed: {}", e); }
                    }
                }
            }
            // Try RSS if enabled and either NPA is disabled or NPA produced no items
            if use_rss && (!use_npa || collected.is_empty()) {
                if let Some(r) = cfg.crawler.rss.as_ref() {
                    if let Ok(re) = regex::Regex::new(&r.regex) {
                        match rss_scanner.fetch(&r.url, &re).await {
                            Ok(mut v) => { if !v.is_empty() { collected.append(&mut v); } }
                            Err(e) => { error!("rss fetch failed: {}", e); }
                        }
                    } else { error!("rss regex invalid"); }
                }
            }
            collected
        };
        {
                let mut published_count: usize = 0;
                for item in items {
                    if let Some(limit) = max_posts_per_run { if published_count >= limit { break; } }
                    let title = if item.title.is_empty() {
                        "Обновление".to_string()
                    } else {
                        item.title.clone()
                    };
                    let _body = item.body.clone();
                    let url = item.url.clone();
                    let project_id = item.project_id.clone();

                    let summary_input = if let Some(pid) = project_id.as_ref() {
                        info!(%url, %title, project_id = %pid, "worker: fetching docx");
                        // If cached, reuse extracted text
                        if let Some(run) = cfg.run.as_ref() {
                            if let Some(cache_dir) = run.cache_dir.as_ref() {
                                if let Ok(Some(_meta)) = load_cache_metadata(cache_dir, pid) {
                                    if let Ok(Some(cached_summary)) = load_cached_summary(cache_dir, pid) {
                                        info!(project_id = %pid, "cache hit: using cached summary");
                                        // publish from cache later
                                        cached_summary
                                    } else {
                                        info!(project_id = %pid, "cache meta found, but summary missing; will regenerate");
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                }
                            } else { String::new() }
                        } else { String::new() }
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "project_id not found in url",
                        ));
                    };

                    // If we had cached summary, publish and continue
                    if !summary_input.is_empty() {
                    let cached_text = match build_post(&cfg, &item, &summary_input) {
                            Ok(s) => s,
                            Err(e) => {
                                error!(%e, "post build failed");
                                // Если пост по закешированному summary не собрался,
                                // убедимся, что summary сохранён в кэше
                                if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                                    let _ = save_cache_artifacts(
                                        cache_dir,
                                        project_id.as_deref().unwrap_or("unknown"),
                                        None,
                                        &item.body,
                                        &summary_input,
                                        "",
                                        &[]
                                    );
                                } }
                                continue;
                            }
                        };
                        // Build publishers
                        let mut publishers: Vec<Box<dyn Publisher>> = Vec::new();
                        if let Some(out) = cfg.output.as_ref() {
                            if out.console_enabled.unwrap_or(false) {
                                publishers.push(Box::new(ConsolePublisher { max_chars: out.console_max_chars }));
                            }
                            if out.file_enabled.unwrap_or(false) {
                                if let Some(path) = out.file_path.clone() {
                                    let append = out.file_append.unwrap_or(true);
                                    publishers.push(Box::new(FilePublisher { path, max_chars: out.file_max_chars, append }));
                                }
                            }
                        }
                        if let (Some(api), Some(chat_id)) = (telegram_api.as_ref(), target_chat_id) {
                            let tmax = cfg.telegram.as_ref().and_then(|t| t.max_chars);
                            publishers.push(Box::new(TelegramPublisherAdapter { api: Arc::clone(api), chat_id, max_chars: tmax }));
                        }
                        if let Some(mstdn) = mastodon.as_ref() {
                            let (vis, lang, spoiler, sensitive) = if let Some(mcfg) = cfg.mastodon.as_ref() {
                                (
                                    mcfg.visibility.clone().or(Some("unlisted".to_string())),
                                    mcfg.language.clone().or(Some("ru".to_string())),
                                    mcfg.spoiler_text.clone().or(Some("Новости".to_string())),
                                    mcfg.sensitive.unwrap_or(false),
                                )
                            } else { (Some("unlisted".to_string()), Some("ru".to_string()), Some("Новости".to_string()), false) };
                            let mmax = cfg.mastodon.as_ref().and_then(|m| m.max_chars);
                            publishers.push(Box::new(MastodonPublisherAdapter { client: Arc::clone(mstdn), visibility: vis, language: lang, spoiler_text: spoiler, sensitive, max_chars: mmax }));
                        }
                        // Filter by already published channels from metadata with logging
                        if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                            if let Ok(Some(meta)) = load_cache_metadata(cache_dir, project_id.as_deref().unwrap_or("unknown")) {
                                let existing: std::collections::HashSet<String> = meta.published_channels.into_iter().collect();
                                let all_names: Vec<String> = publishers.iter().map(|p| p.name().to_string()).collect();
                                let skipped: Vec<String> = all_names
                                    .into_iter()
                                    .filter(|ch| existing.contains(ch))
                                    .collect();
                                if !skipped.is_empty() {
                                    info!(project_id = %project_id.as_deref().unwrap_or("unknown"), skipped_channels = %skipped.join(","), "skip republish: channels already published");
                                }
                                publishers.retain(|p| !existing.contains(p.name()));
                            }
                        } }
                        // Deduplicate publishers by name (avoid double mastodon, etc.)
                        {
                            let mut seen = std::collections::HashSet::new();
                            publishers.retain(|p| seen.insert(p.name().to_string()));
                        }
                        // Publish through remaining publishers; count only real publications
                        let mut published_names: Vec<String> = Vec::new();
                        for p in publishers.iter() {
                            if p.publish(&title, &url, &cached_text).await.is_ok() {
                                published_names.push(p.name().to_string());
                            }
                        }
                        if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                            // сохраняем также пост
                            let _ = save_cache_artifacts(cache_dir, project_id.as_deref().unwrap_or("unknown"), None, &item.body, &summary_input, &cached_text, &published_names);
                            let _ = add_published_channels(cache_dir, project_id.as_deref().unwrap_or("unknown"), &published_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
                        } }
                        if !published_names.is_empty() { published_count += 1; }
                        continue;
                    }

                    // Build MDR text by downloading docx
                    let file_id_tpl = cfg.crawler.file_id.as_ref().map(|f| f.url.clone());
                    let fetcher = DocumentFetcher::new(file_id_tpl);
                    match fetcher.fetch_docx(project_id.as_deref().unwrap_or("unknown")).await {
                        Ok(Some((bytes, text))) => {
                            // Раннее кэширование: сразу сохраняем исходник и извлечённый markdown,
                            // чтобы при дальнейших ошибках не повторять скачивание/парсинг
                            if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                                let _ = save_cache_artifacts(
                                    cache_dir,
                                    project_id.as_deref().unwrap_or("unknown"),
                                    Some(&bytes),
                                    &text,
                                    "",
                                    "",
                                    &[]
                                );
                            } }
                            // Summarize
                            let title_for_prompt = title.clone();
                            let url_for_prompt = url.clone();
                            let text_for_prompt = text.clone();
                            let meta_for_prompt = item.clone();
                            let summarizer_arc = summarizer.clone();
                            let text_for_cache = text_for_prompt.clone();
                            // throttle LLM calls using crawler.poll_delay_secs
                            let llm_delay = cfg.crawler.poll_delay_secs.unwrap_or(0);
                            if llm_delay > 0 { info!(secs = llm_delay, "throttle: sleeping before summarize"); tokio::time::sleep(std::time::Duration::from_secs(llm_delay)).await; }
                            let model_limit = cfg.run.as_ref().and_then(|r| r.model_max_chars);
                            let summary_text = match tokio::time::timeout(
                                std::time::Duration::from_secs(cfg.run.as_ref().and_then(|r| r.summarization_timeout_secs).unwrap_or(120)),
                                async move { summarizer_arc.summarize_with_limit(&title_for_prompt, &text_for_prompt, &url_for_prompt, Some(meta_for_prompt), model_limit).await }
                            ).await {
                                Ok(Ok(s)) => {
                                    // Раннее сохранение summary до публикации
                                    if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                                        let _ = save_cache_artifacts(
                                            cache_dir,
                                            project_id.as_deref().unwrap_or("unknown"),
                                            None,
                                            &text_for_cache,
                                            &s,
                                            "",
                                            &[]
                                        );
                                    } }
                                    s
                                },
                                Ok(Err(e)) => {
                                    error!(%e, "summarizer failed");
                                    // На ошибке тоже убедимся, что markdown уже сохранён (сделано выше).
                                    continue;
                                }
                                Err(_) => {
                                    error!("summarizer timeout");
                                    // На таймауте тоже оставляем ранее сохранённые артефакты.
                                    continue;
                                }
                            };

                            let final_text = match build_post(&cfg, &item, &summary_text) {
                                Ok(s) => s,
                                Err(e) => {
                                    error!(%e, "post build failed");
                                    // Даже если пост собрать не удалось, убедимся что summary сохранён
                                    if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                                        let _ = save_cache_artifacts(
                                            cache_dir,
                                            project_id.as_deref().unwrap_or("unknown"),
                                            None,
                                            &text_for_cache,
                                            &summary_text,
                                            "",
                                            &[]
                                        );
                                    } }
                                    continue;
                                }
                            };

                            // Build publishers
                            let mut publishers: Vec<Box<dyn Publisher>> = Vec::new();
                            if let Some(out) = cfg.output.as_ref() {
                                if out.console_enabled.unwrap_or(false) {
                                    publishers.push(Box::new(ConsolePublisher { max_chars: out.console_max_chars }));
                                }
                                if out.file_enabled.unwrap_or(false) {
                                    if let Some(path) = out.file_path.clone() {
                                    let append = out.file_append.unwrap_or(true);
                                    publishers.push(Box::new(FilePublisher { path, max_chars: out.file_max_chars, append }));
                                    }
                                }
                            }
                            if let (Some(api), Some(chat_id)) = (telegram_api.as_ref(), target_chat_id) {
                                let tmax = cfg.telegram.as_ref().and_then(|t| t.max_chars);
                                publishers.push(Box::new(TelegramPublisherAdapter { api: Arc::clone(api), chat_id, max_chars: tmax }));
                            }
                            if let Some(mstdn) = mastodon.as_ref() {
                            let (vis, lang, spoiler, sensitive) = if let Some(mcfg) = cfg.mastodon.as_ref() {
                                (
                                    mcfg.visibility.clone().or(Some("unlisted".to_string())),
                                    mcfg.language.clone().or(Some("ru".to_string())),
                                    mcfg.spoiler_text.clone().or(Some("Новости".to_string())),
                                    mcfg.sensitive.unwrap_or(false),
                                )
                            } else { (Some("unlisted".to_string()), Some("ru".to_string()), Some("Новости".to_string()), false) };
                            let mmax = cfg.mastodon.as_ref().and_then(|m| m.max_chars);
                            publishers.push(Box::new(MastodonPublisherAdapter { client: Arc::clone(mstdn), visibility: vis, language: lang, spoiler_text: spoiler, sensitive, max_chars: mmax }));
                            }
                            if let Some(mstdn) = mastodon.as_ref() {
                                let (vis, lang, spoiler, sensitive) = if let Some(mcfg) = cfg.mastodon.as_ref() {
                                    (
                                        mcfg.visibility.clone(),
                                        mcfg.language.clone(),
                                        mcfg.spoiler_text.clone().or(Some("Новости".to_string())),
                                        mcfg.sensitive.unwrap_or(false),
                                    )
                                } else { (Some("unlisted".to_string()), Some("ru".to_string()), Some("Новости".to_string()), false) };
                                let mmax = cfg.mastodon.as_ref().and_then(|m| m.max_chars);
                                publishers.push(Box::new(MastodonPublisherAdapter { client: Arc::clone(mstdn), visibility: vis, language: lang, spoiler_text: spoiler, sensitive, max_chars: mmax }));
                            }

                            // Deduplicate publishers by name (avoid double mastodon, etc.)
                            {
                                let mut seen = std::collections::HashSet::new();
                                publishers.retain(|p| seen.insert(p.name().to_string()));
                            }
                            // Publish
                            let mut published_names: Vec<String> = Vec::new();
                            for p in publishers.iter() {
                                if p.publish(&title, &url, &final_text).await.is_ok() {
                                    published_names.push(p.name().to_string());
                                }
                            }

                            if let Some(run) = cfg.run.as_ref() { if let Some(cache_dir) = run.cache_dir.as_ref() {
                                // Финальное сохранение всего комплекта, включая пост
                                let _ = save_cache_artifacts(
                                    cache_dir,
                                    project_id.as_deref().unwrap_or("unknown"),
                                    Some(&bytes),
                                    &text_for_cache,
                                    &summary_text,
                                    &final_text,
                                    &published_names
                                );
                                let _ = add_published_channels(cache_dir, project_id.as_deref().unwrap_or("unknown"), &published_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
                            } }

                            if !published_names.is_empty() { published_count += 1; }
                        }
                        Ok(None) => {
                            info!(project_id = %project_id.as_deref().unwrap_or("unknown"), "docx: no fileId, skip");
                        }
                        Err(e) => {
                            error!(project_id = %project_id.as_deref().unwrap_or("unknown"), error = %e, "docx fetch failed");
                        }
                    }
                }
        }
        if max_posts_per_run.is_some() { return Ok(()); }
    }
}
