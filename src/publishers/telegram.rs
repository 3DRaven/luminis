use async_trait::async_trait;
use reqwest::Client;
use std::env;

use serde::{Deserialize, Serialize};
use crate::traits::telegram_api::TelegramApi;
use crate::traits::publisher::Publisher;
use bon::Builder;

/// A real implementation of the `TelegramApi` trait that sends HTTP requests to the Telegram Bot API.
#[derive(Builder)]
pub struct RealTelegramApi {
    pub client: Client,
    pub base_url: String,
    pub token: String,
    pub chat_id: i64,
    pub max_chars: Option<usize>,
}

impl RealTelegramApi {

    /// Creates a new `RealTelegramApi` instance using environment variables.
    ///
    /// # Environment Variables
    ///
    /// - `TELEGRAM_API_BASE_URL`: Base URL of the Telegram API (e.g., `https://api.telegram.org`)
    /// - `TELEGRAM_BOT_TOKEN`: Telegram bot token
    ///
    /// # Errors
    ///
    /// Returns an error if either environment variable is missing or empty.
    pub fn new_from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let base_url = env::var("TELEGRAM_API_BASE_URL")
            .map_err(|_| "Environment variable TELEGRAM_API_BASE_URL is not set or empty")?;
        if base_url.trim().is_empty() {
            return Err("Environment variable TELEGRAM_API_BASE_URL cannot be empty".into());
        }

        let token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| "Environment variable TELEGRAM_BOT_TOKEN is not set or empty")?;
        if token.trim().is_empty() {
            return Err("Environment variable TELEGRAM_BOT_TOKEN cannot be empty".into());
        }

        Ok(Self {
            client: Client::new(),
            base_url,
            token,
            chat_id: 0, // Will be set later
            max_chars: None,
        })
    }
}

#[async_trait]
impl TelegramApi for RealTelegramApi {
    /// Sends a message to a Telegram chat using the Telegram Bot API.
    ///
    /// # Arguments
    ///
    /// * `chat_id` - Telegram chat ID to send the message to.
    /// * `text` - Message text to send.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or `Err(String)` with an error message on failure.
    async fn send_telegram_message(&self, chat_id: i64, text: String) -> Result<(), String> {
        let url = format!("{}/bot{}/sendMessage", self.base_url, self.token);
        let message = SendMessageRequest { chat_id, text };

        let response = self
            .client
            .post(&url)
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "HTTP error sending Telegram message");
                format!("HTTP error: {}", e)
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("Telegram API error {}: {}", status, body))
        }
    }
    
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    
    fn base_url(&self) -> &str {
        &self.base_url
    }
    
    fn token(&self) -> &str {
        &self.token
    }
}

#[async_trait]
impl Publisher for RealTelegramApi {
    fn name(&self) -> &str { "telegram" }
    async fn publish(&self, _title: &str, _url: &str, text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cut = if let Some(maxc) = self.max_chars { 
            super::utils::trim_with_ellipsis(text, maxc) 
        } else { 
            text.to_string() 
        };
        let _ = self.send_telegram_message(self.chat_id, cut).await;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
}
