use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::models::types::CrawlItem;

#[async_trait]
pub trait Crawler: Send + Sync {
    async fn fetch_stream(&self, sender: mpsc::Sender<CrawlItem>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}


