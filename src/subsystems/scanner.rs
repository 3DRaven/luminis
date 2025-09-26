use std::time::Duration;

use anyhow::Result;
use backon::{ExponentialBuilder, Retryable};
use bon::Builder;
use tokio::sync::mpsc;
use tokio_graceful_shutdown::{FutureExt, SubsystemHandle};
use tokio_graceful_shutdown::errors::CancelledByShutdown;
use tracing::{error, info};

use crate::models::types::CrawlItem;
use crate::crawlers::NpaListCrawler;
use crate::models::config::AppConfig;
use crate::services::channels::ChannelManager;
use crate::traits::cache_manager::CacheManager;
use crate::traits::crawler::Crawler;
use std::sync::Arc;

#[derive(Builder)]
pub struct ScannerSubsystem {
    pub(crate) config: AppConfig,
    pub(crate) req_timeout: Duration,
    pub(crate) sender: mpsc::Sender<CrawlItem>,
    pub(crate) cache_manager: Arc<dyn CacheManager>,
}

impl ScannerSubsystem {
    pub async fn run(self, subsys: SubsystemHandle) -> std::io::Result<()> {
        info!("Starting NPAListCrawler subsystem");

        let fut = async {
            let npa_interval_secs = self
                .config
                .crawler
                .npalist
                .as_ref()
                .and_then(|n| n.interval_seconds)
                .unwrap_or(300);

            let max_retry_attempts = self.config.crawler.max_retry_attempts.unwrap_or(0);
            let mut interval = tokio::time::interval(Duration::from_secs(npa_interval_secs));
            
            // Создаем ChannelManager для получения включенных каналов
            let channel_manager = ChannelManager::builder().config(&self.config).build();
            let enabled_channels: Vec<crate::models::channel::PublisherChannel> = channel_manager.get_enabled_channels()
                .iter()
                .map(|config| config.channel)
                .collect();

            loop {
                interval.tick().await;

                if let Some(npa) = self
                    .config
                    .crawler
                    .npalist
                    .as_ref()
                    .filter(|n| n.enabled.unwrap_or(true))
                {
                    let npa_re = npa
                        .regex
                        .as_ref()
                        .and_then(|s| regex::Regex::new(s).ok());

                    let poll_delay = Duration::from_secs(self.config.crawler.poll_delay_secs.unwrap_or(0));
                    
                    // Попытка получить данные с retry логикой (потоковая отправка)
                    let result = Self::try_fetch_data_stream_with_retry(
                        &self.config,
                        &self.sender,
                        self.req_timeout,
                        Arc::clone(&self.cache_manager),
                        npa.url.clone(),
                        npa.limit,
                        npa_re.clone(),
                        poll_delay,
                        max_retry_attempts,
                        enabled_channels.clone(),
                    ).await;

                    match result {
                        Ok(()) => {
                            info!("crawler: streaming completed successfully");
                        }
                        Err(e) => {
                            error!(error = %e, "All crawlers failed after retries, shutting down");
                            subsys.request_shutdown();
                            break;
                        }
                    }
                }
            }

            Ok::<(), std::io::Error>(())
        };

        match fut.cancel_on_shutdown(&subsys).await {
            Ok(Ok(())) => info!("NPAListCrawler subsystem finished"),
            Ok(Err(e)) => return Err(e),
            Err(CancelledByShutdown) => info!("NPAListCrawler subsystem cancelled by shutdown"),
        }

        Ok(())
    }

    async fn try_fetch_data_stream_with_retry(
        _config: &AppConfig,
        sender: &mpsc::Sender<CrawlItem>,
        req_timeout: Duration,
        cache_manager: Arc<dyn CacheManager>,
        npa_url: String,
        npa_limit: Option<u32>,
        npa_re: Option<regex::Regex>,
        poll_delay: Duration,
        max_retry_attempts: u64,
        enabled_channels: Vec<crate::models::channel::PublisherChannel>,
    ) -> Result<()> {
        let fetch_data = || async {
            // Сначала пытаемся NPA краулер с потоковой отправкой
            let npa_result: Result<()> = match NpaListCrawler::builder()
                .url_template(npa_url.clone())
                .maybe_limit_opt(npa_limit)
                .maybe_project_id_re(npa_re.clone())
                .timeout(req_timeout)
                .cache_manager(Arc::clone(&cache_manager))
                .poll_delay(poll_delay)
                .enabled_channels(enabled_channels.clone())
                .build() {
                Ok(npa_crawler) => match npa_crawler.fetch_stream(sender.clone()).await {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(e) => Err(anyhow::anyhow!("NPA fetch_stream failed: {}", e))
                },
                Err(e) => Err(anyhow::anyhow!("NPA crawler creation failed: {}", e))
            };

            // Если NPA не сработал, возвращаем ошибку
            npa_result
        };

        // Настраиваем retry стратегию
        let mut builder = ExponentialBuilder::default();
        if max_retry_attempts > 0 {
            builder = builder.with_max_times(max_retry_attempts as usize);
        }

        fetch_data
            .retry(builder)
            .sleep(tokio::time::sleep)
            .when(|e: &anyhow::Error| {
                // Повторяем попытку если NPA краулер упал
                e.to_string().contains("NPA")
            })
            .notify(|err: &anyhow::Error, dur: Duration| {
                info!(
                    "Retrying crawler after {:?} due to error: {}",
                    dur,
                    err
                );
            })
            .await
    }

}


