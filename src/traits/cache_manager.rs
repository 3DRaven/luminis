use async_trait::async_trait;
use crate::models::types::CacheMetadata;
use crate::models::channel::PublisherChannel;
use crate::models::types::{SummaryText, PostText, MetadataItem};

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
        published_channels: &[PublisherChannel],
        crawl_metadata: &[MetadataItem],
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
        new_channels: &[PublisherChannel],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Добавляет один канал в список опубликованных
    async fn add_published_channel(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Атомарно обновляет данные канала (суммаризацию, пост и статус публикации)
    async fn update_channel_data(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        summary_text: Option<&str>,
        post_text: Option<&str>,
        is_published: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли данные в кэше
    async fn has_data(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли суммаризация в кэше
    async fn has_summary(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, опубликован ли проект в указанном канале
    async fn is_published_in_channel(
        &self,
        project_id: &str,
        channel: PublisherChannel,
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
        channel: PublisherChannel,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает суммаризацию для конкретного канала
    async fn load_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<Option<SummaryText>, Box<dyn std::error::Error + Send + Sync>>;

    /// Обновляет суммаризацию для конкретного канала
    async fn update_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        summary_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, есть ли пост для конкретного канала
    async fn has_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает пост для конкретного канала
    async fn load_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<Option<PostText>, Box<dyn std::error::Error + Send + Sync>>;

    /// Обновляет пост для конкретного канала
    async fn update_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        post_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Загружает manifest
    async fn load_manifest(&self) -> Result<crate::models::types::Manifest, Box<dyn std::error::Error + Send + Sync>>;

    /// Сохраняет manifest
    async fn save_manifest(&self, manifest: &crate::models::types::Manifest) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Обновляет min_published_project_id в manifest
    async fn update_min_published_project_id(&self, min_id: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Атомарно обновляет все данные каналов для проекта
    async fn update_all_channels_data(
        &self,
        project_id: &str,
        channel_data: &[(crate::models::channel::PublisherChannel, &str, &str)],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Проверяет, был ли элемент полностью опубликован во все ожидаемые каналы
    async fn is_fully_published(&self, project_id: &str, enabled_channels: &[crate::models::channel::PublisherChannel]) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}
