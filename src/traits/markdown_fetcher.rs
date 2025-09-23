use async_trait::async_trait;

/// Общий интерфейс для получения markdown-текста и исходных байт документа по идентификатору проекта.
#[async_trait]
pub trait MarkdownFetcher: Send + Sync {
    /// Возвращает пару (сырые байты исходного файла, извлечённый markdown) или None, если файла нет.
    async fn fetch_markdown(
        &self,
        project_id: &str,
    ) -> Result<Option<(Vec<u8>, String)>, Box<dyn std::error::Error + Send + Sync>>;
}


