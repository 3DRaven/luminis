use async_trait::async_trait;
use std::error::Error;

use super::utils::trim_with_ellipsis;
use crate::traits::publisher::Publisher;

pub struct ConsolePublisher {
    pub max_chars: Option<usize>,
}

#[async_trait]
impl Publisher for ConsolePublisher {
    fn name(&self) -> &str { "console" }
    async fn publish(&self, title: &str, url: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let final_text = if let Some(maxc) = self.max_chars { trim_with_ellipsis(text, maxc) } else { text.to_string() };
        #[cfg(test)]
        {
            use super::utils::CONSOLE_TEST_SINK;
            CONSOLE_TEST_SINK.lock().unwrap().push(final_text.clone());
        }
        #[cfg(not(test))]
        {
            println!("{}", final_text);
        }
        // Still add a structured log entry with lengths for observability
        tracing::info!(title_len = title.len(), url_len = url.len(), text_len = final_text.len(), "console publisher output");
        Ok(())
    }
}
