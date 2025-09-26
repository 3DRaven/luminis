use std::sync::Arc;

use bon::Builder;
use tokio::sync::mpsc;
use tokio_graceful_shutdown::{FutureExt, SubsystemHandle};
use tokio_graceful_shutdown::errors::CancelledByShutdown;
use tracing::info;

use crate::models::types::CrawlItem;
use crate::services::summarizer::Summarizer;
use crate::services::worker::Worker;
use crate::traits::cache_manager::CacheManager;
use crate::traits::telegram_api::TelegramApi;
use crate::models::config::AppConfig;

#[derive(Builder)]
pub struct WorkerSubsystem {
    pub(crate) config: AppConfig,
    pub(crate) summarizer: Arc<Summarizer>,
    pub(crate) telegram_api: Option<Arc<dyn TelegramApi>>,
    pub(crate) target_chat_id: Option<i64>,
    pub(crate) cache_manager: Arc<dyn CacheManager>,
    pub(crate) receiver: mpsc::Receiver<CrawlItem>,
}

impl WorkerSubsystem {
    pub async fn run(self, subsys: SubsystemHandle) -> std::io::Result<()> {
        info!("Starting Worker subsystem");

        let worker = Worker::builder()
            .config(self.config.clone())
            .summarizer(Arc::clone(&self.summarizer))
            .maybe_telegram_api(self.telegram_api.as_ref().map(Arc::clone))
            .maybe_target_chat_id(self.target_chat_id.clone())
            .cache_manager(Arc::clone(&self.cache_manager))
            .build()
            .await?;

        let max_posts_per_run = self
            .config
            .run
            .as_ref()
            .and_then(|r| r.max_posts_per_run);

        let fut = async move {
            let mut rx = self.receiver;
            let mut published_count = 0;

            loop {
                // Ожидаем сообщения из канала без таймаутов
                match rx.recv().await {
                    Some(item) => {
                        info!("received item from npa crawler: {}", item.title);
                        let count = worker.process_item(item).await?;
                        published_count += count;
                        
                        // Если задан лимит постов, завершаем после обработки
                        if let Some(limit) = max_posts_per_run {
                            if published_count >= limit {
                                break;
                            }
                        }
                    }
                    None => {
                        info!("npa crawler channel closed, worker shutting down");
                        break;
                    }
                }
            }

            Ok::<(), std::io::Error>(())
        };

        match fut.cancel_on_shutdown(&subsys).await {
            Ok(Ok(())) => {
                info!("Worker subsystem finished");
                // Запрашиваем завершение прочих подсистем
                subsys.request_shutdown();
            }
            Ok(Err(e)) => return Err(e),
            Err(CancelledByShutdown) => info!("Worker subsystem cancelled by shutdown"),
        }

        Ok(())
    }
}


