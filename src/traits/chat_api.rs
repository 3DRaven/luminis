use async_trait::async_trait;

/// Defines the interface for a chat-based language model API (e.g., OpenAI, LocalAI).
///
/// This trait allows consumers to abstract over different backend implementations
/// (e.g., real HTTP clients, mocks for testing).
///
/// Any implementation must be thread-safe (`Send + Sync`) and provide an asynchronous
/// method for sending prompts and receiving model-generated responses.
#[async_trait]
pub trait ChatApi: Send + Sync {
    /// Sends a prompt to a chat API and returns the assistant's response.
    async fn call_chat_api(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}


