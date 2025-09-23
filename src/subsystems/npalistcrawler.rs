use std::time::Duration;

use anyhow::Result;
use backon::{ExponentialBuilder, Retryable};
use bon::Builder;
use tokio::sync::mpsc;
use tokio_graceful_shutdown::{FutureExt, SubsystemHandle};
use tokio_graceful_shutdown::errors::CancelledByShutdown;
use tracing::{error, info};

use crate::services::crawler::{CrawlItem, NpaListCrawler, RssCrawler};
use crate::services::settings::AppConfig;
use crate::traits::cache_manager::CacheManager;
use std::sync::Arc;
use crate::traits::crawler::Crawler;

#[derive(Builder)]
pub struct NpaListCrawlerSubsystem {
    pub(crate) config: AppConfig,
    pub(crate) req_timeout: Duration,
    pub(crate) sender: mpsc::Sender<Vec<CrawlItem>>,
    pub(crate) cache_manager: Arc<dyn CacheManager>,
}

impl NpaListCrawlerSubsystem {
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
                    
                    // Попытка получить данные с retry логикой
                    let result = Self::try_fetch_data_with_retry(
                        &self.config,
                        &self.sender,
                        self.req_timeout,
                        Arc::clone(&self.cache_manager),
                        npa.url.clone(),
                        npa.limit,
                        npa_re.clone(),
                        poll_delay,
                        max_retry_attempts,
                    ).await;

                    match result {
                        Ok(items) => {
                            if !items.is_empty() {
                                info!(count = items.len(), "crawler: sending items");
                                if let Err(_) = self.sender.send(items).await {
                                    info!("crawler: receiver dropped, stopping");
                                    break;
                                }
                            } else {
                                info!("crawler: no items");
                            }
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

    async fn try_fetch_data_with_retry(
        config: &AppConfig,
        sender: &mpsc::Sender<Vec<CrawlItem>>,
        req_timeout: Duration,
        cache_manager: Arc<dyn CacheManager>,
        npa_url: String,
        npa_limit: Option<u32>,
        npa_re: Option<regex::Regex>,
        poll_delay: Duration,
        max_retry_attempts: u64,
    ) -> Result<Vec<CrawlItem>> {
        let fetch_data = || async {
            // Сначала пытаемся NPA краулер
            let npa_result: Result<Vec<CrawlItem>> = match NpaListCrawler::builder()
                .url_template(npa_url.clone())
                .maybe_limit_opt(npa_limit)
                .maybe_project_id_re(npa_re.clone())
                .timeout(req_timeout)
                .cache_manager(Arc::clone(&cache_manager))
                .poll_delay(poll_delay)
                .build() {
                Ok(npa_crawler) => match npa_crawler.fetch().await {
                    Ok(items) => {
                        if !items.is_empty() {
                            return Ok(items);
                        }
                        Err(anyhow::anyhow!("NPA returned no items"))
                    }
                    Err(e) => Err(anyhow::anyhow!("NPA fetch failed: {}", e))
                },
                Err(e) => Err(anyhow::anyhow!("NPA crawler creation failed: {}", e))
            };

            // Если NPA не сработал, пробуем RSS fallback
            match Self::handle_rss_fallback(config, sender, req_timeout).await {
                Ok(items) => Ok(items),
                Err(rss_err) => {
                    // Оба краулера упали
                    Err(anyhow::anyhow!("Both NPA and RSS crawlers failed. NPA: {}, RSS: {}", npa_result.unwrap_err(), rss_err))
                }
            }
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
                // Повторяем попытку если оба краулера упали
                e.to_string().contains("Both NPA and RSS crawlers failed")
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

    async fn handle_rss_fallback(
        config: &AppConfig,
        tx: &mpsc::Sender<Vec<CrawlItem>>,
        req_timeout: Duration,
    ) -> Result<Vec<CrawlItem>> {
        if let Some(r) = config
            .crawler
            .rss
            .as_ref()
            .filter(|r| r.enabled.unwrap_or(true))
        {
            if let Ok(re) = regex::Regex::new(&r.regex) {
                match RssCrawler::builder().url(r.url.clone()).regex(re).timeout(req_timeout).build() {
                    Ok(rss_crawler) => match rss_crawler.fetch().await {
                        Ok(items) => {
                            if !items.is_empty() {
                                info!(count = items.len(), "rss fallback: sending items");
                                let _ = tx.send(items.clone()).await;
                                return Ok(items); // RSS успешно получил данные
                            }
                            return Err(anyhow::anyhow!("RSS returned no items")); // RSS не вернул данные
                        }
                        Err(e) => {
                            error!(error = %e, "rss fallback failed");
                            return Err(anyhow::anyhow!("RSS fetch failed: {}", e)); // RSS не смог получить данные
                        }
                    },
                    Err(e) => {
                        error!(error = %e, "rss crawler creation failed");
                        return Err(anyhow::anyhow!("RSS crawler creation failed: {}", e)); // RSS краулер не создался
                    }
                }
            }
        }
        Err(anyhow::anyhow!("RSS disabled or invalid regex")) // RSS отключен или regex невалидный
    }
}


