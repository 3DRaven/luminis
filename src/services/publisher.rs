use async_trait::async_trait;
use std::error::Error;
use std::sync::Arc;

use crate::traits::telegram_api::TelegramApi;
use crate::services::mastodon::MastodonPublisher;
use mastodon_async::Language;
use tracing::{info, error};

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

pub use crate::traits::publisher::Publisher;

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

pub struct FilePublisher {
    pub path: String,
    pub max_chars: Option<usize>,
    pub append: bool,
}

#[async_trait]
impl Publisher for FilePublisher {
    fn name(&self) -> &str { "file" }
    async fn publish(&self, _title: &str,_urll: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let final_text = if let Some(maxc) = self.max_chars { trim_with_ellipsis(text, maxc) } else { text.to_string() };
        let p = std::path::Path::new(&self.path);
        if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
        if self.append {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
            writeln!(f, "{}", final_text)?;
        } else {
            std::fs::write(p, format!("{}\n", final_text))?;
        }
        Ok(())
    }
}

pub struct TelegramPublisherAdapter {
    pub api: Arc<dyn TelegramApi>,
    pub chat_id: i64,
    pub max_chars: Option<usize>,
}

#[async_trait]
impl Publisher for TelegramPublisherAdapter {
    fn name(&self) -> &str { "telegram" }
    async fn publish(&self, _title: &str, _url: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cut = if let Some(maxc) = self.max_chars { trim_with_ellipsis(text, maxc) } else { text.to_string() };
        let _ = self.api.send_telegram_message(self.chat_id, cut).await;
        Ok(())
    }
}

pub struct MastodonPublisherAdapter {
    pub client: Arc<MastodonPublisher>,
    pub visibility: Option<String>,
    pub language: Option<String>,
    pub spoiler_text: Option<String>,
    pub sensitive: bool,
    pub max_chars: Option<usize>,
}

#[async_trait]
impl Publisher for MastodonPublisherAdapter {
    fn name(&self) -> &str { "mastodon" }
    async fn publish(&self, _title: &str, _url: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cut = if let Some(maxc) = self.max_chars { trim_with_ellipsis(text, maxc) } else { text.to_string() };
        let lang = self.language.as_deref().unwrap_or("ru");
        let lang = Language::from_639_1(lang);
        let vis = self.visibility.as_deref();
        let spoiler = self.spoiler_text.as_deref().filter(|s| !s.is_empty());
        info!(
            text_len = cut.len(), visibility = ?vis, language = ?self.language, spoiler = ?spoiler,
            sensitive = self.sensitive, "mastodon: publish start"
        );
        match self.client.post_status_advanced(&cut, vis, lang, spoiler, self.sensitive).await {
            Ok(()) => { info!("mastodon: publish success"); Ok(()) }
            Err(e) => { error!(error = %e, "mastodon: publish failed"); Err(e) }
        }
    }
}

#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
static CONSOLE_TEST_SINK: once_cell::sync::Lazy<Mutex<Vec<String>>> = once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Read;

    #[tokio::test]
    async fn trims_with_ellipsis_basic() {
        let s = "абвгд";
        assert_eq!(trim_with_ellipsis(s, 0), "");
        assert_eq!(trim_with_ellipsis(s, 1), "…");
        assert_eq!(trim_with_ellipsis(s, 2), "а…");
        assert_eq!(trim_with_ellipsis(s, 3), "аб…");
        assert_eq!(trim_with_ellipsis(s, 5), "абвгд");
        assert_eq!(trim_with_ellipsis(s, 10), "абвгд");
    }

    #[tokio::test]
    async fn console_and_file_outputs_match() {
        let long_text = "Это очень длинный текст для проверки обрезки. Здесь много символов и нам важно убедиться, что в конце появляется троеточие при превышении лимита.";
        let maxc = 30usize;

        // Console publish (captured into test sink)
        let console = ConsolePublisher { max_chars: Some(maxc) };
        console.publish("Title", "https://example.com", long_text).await.unwrap();

        // File publish to temp file
        let tf = NamedTempFile::new().unwrap();
        let path = tf.path().to_path_buf();
        let fp = FilePublisher { path: path.to_string_lossy().to_string(), max_chars: Some(maxc), append: false };
        fp.publish("Title", "https://example.com", long_text).await.unwrap();

        // Read file content
        let mut content = String::new();
        std::fs::File::open(path).unwrap().read_to_string(&mut content).unwrap();
        // Drop trailing newline for comparison
        if content.ends_with('\n') { content.pop(); if content.ends_with('\r') { content.pop(); } }

        let sink = CONSOLE_TEST_SINK.lock().unwrap();
        let last_console = sink.last().expect("console sink has an entry").clone();

        assert_eq!(last_console, content, "Console and file outputs must be identical");
    }
}


