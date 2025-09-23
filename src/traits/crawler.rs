use async_trait::async_trait;

use crate::services::crawler::CrawlItem;

#[async_trait]
pub trait Crawler: Send + Sync {
    async fn fetch(&self) -> Result<Vec<CrawlItem>, Box<dyn std::error::Error + Send + Sync>>;
}


