pub mod services;
pub mod traits;
pub mod subsystems;
pub mod models;
pub mod crawlers;
pub mod publishers;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_graceful_shutdown::{SubsystemBuilder, Toplevel};

use crate::traits::chat_api::ChatApi;
use crate::services::chat_api_local::LocalChatApi;
use crate::models::config::AppConfig;
use crate::services::settings::load_config;
use crate::services::summarizer::Summarizer;
use crate::traits::telegram_api::TelegramApi;
use crate::publishers::RealTelegramApi;
use reqwest::Client;
use crate::traits::cache_manager::CacheManager;
use crate::services::cache_manager_impl::FileSystemCacheManager;
use crate::subsystems::scanner::ScannerSubsystem;
use crate::subsystems::worker::WorkerSubsystem;

/// High-level entrypoint: load config, init logging, run worker
pub async fn run_with_config_path(path: &str, log_file: Option<&str>) -> std::io::Result<()> {
    // Load YAML config
    let cfg: AppConfig = load_config(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to load {}: {}", path, e)))?;

    // Initialize structured logging (default to info if RUST_LOG not set)
    let log_spec = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    
    // Проверяем, нужно ли логирование в файл
    if let Some(log_path) = log_file {
        // Логирование в файл и консоль
        let file_appender = tracing_appender::rolling::daily(
            std::path::Path::new(&log_path).parent().unwrap_or(std::path::Path::new("/tmp")),
            std::path::Path::new(&log_path).file_name().unwrap_or(std::ffi::OsStr::new("luminis.log"))
        );
        
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(log_spec))
            .with_target(false)
            .compact()
            .with_writer(non_blocking)
            .try_init();
    } else {
        // Только консольное логирование
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(log_spec))
            .with_target(false)
            .compact()
            .try_init();
    }

    // Initialize shared services from config
    let chat_api: Arc<dyn ChatApi> = Arc::new(LocalChatApi::from_config(&cfg.llm));
    let summarizer = Arc::new(Summarizer::builder()
        .chat_api(Arc::clone(&chat_api))
        .hard_max_chars(600)
        .sample_percent(0.05)
        .max_retry_attempts(3)
        .retry_delay_secs(2)
        .build()
        .with_config(&cfg));

    let (telegram_api, target_chat_id) = if let Some(tg) = cfg.telegram.clone().filter(|t| t.enabled) {
        let api: Arc<dyn TelegramApi> = Arc::new(RealTelegramApi {
            client: Client::new(),
            base_url: tg.api_base_url,
            token: tg.bot_token,
            chat_id: tg.target_chat_id,
            max_chars: tg.max_chars,
        });
        (Some(api), Some(tg.target_chat_id))
    } else {
        (None, None)
    };

    // Ensure post template is provided
    if cfg.run.as_ref().and_then(|r| r.post_template.as_ref()).is_none() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "run.post_template is required in config (no fallback post formatting)"));
    }

    let req_timeout = Duration::from_secs(cfg.crawler.request_timeout_secs.unwrap_or(30));

    // Initialize cache manager
    let cache_dir = cfg
        .run
        .as_ref()
        .and_then(|r| r.cache_dir.as_ref())
        .map(|s| s.clone())
        .unwrap_or_else(|| "./cache".to_string());
    let cache_manager: Arc<dyn CacheManager> = Arc::new(FileSystemCacheManager::builder().cache_dir(cache_dir).build());

    // Channel between crawler and worker (single items)
    let (tx, rx) = mpsc::channel(10);

    // Build subsystems
    let npa_subsystem = ScannerSubsystem::builder()
        .config(cfg.clone())
        .req_timeout(req_timeout)
        .sender(tx)
        .cache_manager(Arc::clone(&cache_manager))
        .build();

    let worker_subsystem = if let (Some(api), Some(chat_id)) = (telegram_api.clone(), target_chat_id) {
        WorkerSubsystem::builder()
            .config(cfg.clone())
            .summarizer(Arc::clone(&summarizer))
            .telegram_api(api)
            .target_chat_id(chat_id)
            .cache_manager(Arc::clone(&cache_manager))
            .receiver(rx)
            .build()
    } else if let Some(api) = telegram_api.clone() {
        WorkerSubsystem::builder()
            .config(cfg.clone())
            .summarizer(Arc::clone(&summarizer))
            .telegram_api(api)
            .cache_manager(Arc::clone(&cache_manager))
            .receiver(rx)
            .build()
    } else if let Some(chat_id) = target_chat_id {
        WorkerSubsystem::builder()
            .config(cfg.clone())
            .summarizer(Arc::clone(&summarizer))
            .target_chat_id(chat_id)
            .cache_manager(Arc::clone(&cache_manager))
            .receiver(rx)
            .build()
    } else {
        WorkerSubsystem::builder()
            .config(cfg.clone())
            .summarizer(Arc::clone(&summarizer))
            .cache_manager(Arc::clone(&cache_manager))
            .receiver(rx)
            .build()
    };

    // Setup and execute subsystem tree
    Toplevel::new(|s| async move {
        s.start(SubsystemBuilder::new("NPAListCrawler", |h| npa_subsystem.run(h)));
        s.start(SubsystemBuilder::new("Worker", |h| worker_subsystem.run(h)));
    })
    .catch_signals()
    .handle_shutdown_requests(Duration::from_secs(5))
    .await
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("shutdown error: {}", e)))
}

// run_worker оставлен в истории как документационный артефакт и заменён подсистемной моделью
