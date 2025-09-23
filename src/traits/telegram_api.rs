use async_trait::async_trait;

/// `TelegramApi` defines an interface for sending messages via the Telegram Bot API.
///
/// This trait allows different implementations, including mock implementations for testing
/// and real ones that send actual HTTP requests.
#[async_trait]
pub trait TelegramApi: Send + Sync {
    /// Sends a text message to a specified Telegram chat.
    async fn send_telegram_message(&self, chat_id: i64, text: String) -> Result<(), String>;
}


