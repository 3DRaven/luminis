use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait Publisher: Send + Sync {
    fn name(&self) -> &str;
    async fn publish(&self, title: &str, url: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>>;
}


