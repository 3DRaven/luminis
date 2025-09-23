use async_trait::async_trait;
use crate::services::documents::CacheMetadata;

/// Trait для управления кэшем артефактов обработки
#[async_trait]
pub trait CacheManager: Send + Sync {
    /// Сохраняет артефакты в кэш
    async fn save_artifacts(
        &self,
        project_id: &str,
        docx_bytes: Option<&[u8]>,
        markdown_text: &str,
        summary_text: &str,
        post_text: &str,
        published_channels: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает метаданные кэша для проекта
    async fn load_metadata(
        &self,
        project_id: &str,
    ) -> Result<Option<CacheMetadata>, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает кэшированную суммаризацию
    async fn load_summary(
        &self,
        project_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает кэшированные данные (markdown)
    async fn load_cached_data(
        &self,
        project_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Добавляет каналы в список опубликованных
    async fn add_published_channels(
        &self,
        project_id: &str,
        new_channels: &[&str],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли данные в кэше
    async fn has_data(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли суммаризация в кэше
    async fn has_summary(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, опубликован ли проект в указанном канале
    async fn is_published_in_channel(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Получает список опубликованных каналов
    async fn get_published_channels(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли суммаризация для конкретного канала
    async fn has_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает суммаризацию для конкретного канала
    async fn load_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Сохраняет суммаризацию для конкретного канала
    async fn save_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
        summary_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли пост для конкретного канала
    async fn has_channel_post(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает пост для конкретного канала
    async fn load_channel_post(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;

    /// Сохраняет пост для конкретного канала
    async fn save_channel_post(
        &self,
        project_id: &str,
        channel: &str,
        post_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
