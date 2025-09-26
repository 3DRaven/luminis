use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::fs;
use serde_json;
use bon::Builder;

use crate::traits::cache_manager::CacheManager;
use crate::models::types::CacheMetadata;
use crate::models::channel::PublisherChannel;
use crate::models::types::{CreatedAt, SummaryText, PostText};

/// Реализация CacheManager для файловой системы
#[derive(Builder)]
pub struct FileSystemCacheManager {
    cache_dir: String,
}

impl FileSystemCacheManager {
    fn project_dir(&self, project_id: &str) -> PathBuf {
        let mut p = PathBuf::from(&self.cache_dir);
        p.push(project_id);
        p
    }

    fn meta_path_for(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("metadata.json")
    }
}

#[async_trait]
impl CacheManager for FileSystemCacheManager {
    async fn save_artifacts(
        &self,
        project_id: &str,
        docx_bytes: Option<&[u8]>,
        markdown_text: &str,
        _summary_text: &str,
        _post_text: &str,
        published_channels: &[PublisherChannel],
        crawl_metadata: &[crate::models::types::MetadataItem],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base = self.project_dir(project_id);
        fs::create_dir_all(&base)?;
        let ts: CreatedAt = chrono::Utc::now().to_rfc3339().into();

        // per-project subdir layout
        let docx_path = base.join("source.docx");
        let md_path = base.join("extracted.md");
        let meta_path = base.join("metadata.json");

        if let Some(bytes) = docx_bytes {
            fs::write(&docx_path, bytes)?;
        }
        fs::write(&md_path, markdown_text)?;

        // Загружаем существующие метаданные, если они есть, чтобы сохранить published_channels
        let (existing_published_channels, existing_channel_summaries, existing_channel_posts, existing_crawl_metadata) = if meta_path.exists() {
            let data = fs::read_to_string(&meta_path).ok();
            if let Some(meta) = data.and_then(|d| serde_json::from_str::<CacheMetadata>(&d).ok()) {
                (meta.published_channels, meta.channel_summaries, meta.channel_posts, meta.crawl_metadata)
            } else {
                (vec![], std::collections::HashMap::new(), std::collections::HashMap::new(), vec![])
            }
        } else {
            (vec![], std::collections::HashMap::new(), std::collections::HashMap::new(), vec![])
        };

        let meta = CacheMetadata {
            project_id: project_id.to_string().into(),
            docx_path: docx_path.to_string_lossy().to_string().into(),
            markdown_path: md_path.to_string_lossy().to_string().into(),
            // Сохраняем существующие published_channels, если передан пустой список
            published_channels: if published_channels.is_empty() {
                existing_published_channels
            } else {
                published_channels.to_vec()
            },
            created_at: ts.into(),
            channel_summaries: existing_channel_summaries,
            channel_posts: existing_channel_posts,
            // Сохраняем метаданные из crawler, если переданы, иначе сохраняем существующие
            crawl_metadata: if crawl_metadata.is_empty() {
                existing_crawl_metadata
            } else {
                crawl_metadata.to_vec()
            },
        };
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&meta_path, json)?;
        Ok(())
    }

    async fn load_metadata(
        &self,
        project_id: &str,
    ) -> Result<Option<CacheMetadata>, Box<dyn std::error::Error + Send + Sync>> {
        // new layout first
        let p = self.meta_path_for(project_id);
        let data = if p.exists() {
            fs::read_to_string(p)?
        } else {
            // legacy fallback
            let legacy = Path::new(&self.cache_dir).join(format!("{}_metadata.json", project_id));
            if !legacy.exists() {
                return Ok(None);
            }
            fs::read_to_string(legacy)?
        };
        match serde_json::from_str::<CacheMetadata>(&data) {
            Ok(m) => Ok(Some(m)),
            Err(_) => Ok(None),
        }
    }

    async fn load_summary(
        &self,
        project_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        // Читаем из metadata.json
        let meta = self.load_metadata(project_id).await?;
        if let Some(meta) = meta {
            // Возвращаем первую доступную суммаризацию из каналов
            if let Some((_, summary)) = meta.channel_summaries.iter().next() {
                return Ok(Some(summary.as_str().to_string()));
            }
        }
        
        // Legacy fallback - проверяем старый файл summary.txt
        let legacy = Path::new(&self.cache_dir).join(format!("{}_summary.txt", project_id));
        if legacy.exists() {
            return Ok(Some(fs::read_to_string(legacy)?));
        }
        
        Ok(None)
    }

    async fn load_cached_data(
        &self,
        project_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        // new layout first
        let p = self.project_dir(project_id).join("extracted.md");
        let s = if p.exists() {
            fs::read_to_string(p)?
        } else {
            // legacy fallback
            let legacy = Path::new(&self.cache_dir).join(format!("{}_extracted.md", project_id));
            if !legacy.exists() {
                return Ok(None);
            }
            fs::read_to_string(legacy)?
        };
        Ok(Some(s))
    }

    async fn add_published_channels(
        &self,
        project_id: &str,
        new_channels: &[PublisherChannel],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or(CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        for ch in new_channels {
            if !meta.published_channels.iter().any(|c| c == ch) {
                meta.published_channels.push(*ch);
            }
        }
        let out = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(p, out)?;
        Ok(())
    }

    async fn add_published_channel(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            // Читаем существующие данные или создаем новые только если файл пуст/поврежден
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or_else(|_| {
                // При ошибке парсинга НЕ перезаписываем весь файл - только добавляем канал
                CacheMetadata {
                    project_id: project_id.to_string().into(),
                    docx_path: String::new().into(),
                    markdown_path: String::new().into(),
                    published_channels: vec![],
                    created_at: chrono::Utc::now().to_rfc3339().into(),
                    channel_summaries: std::collections::HashMap::new(),
                    channel_posts: std::collections::HashMap::new(),
                    crawl_metadata: vec![],
                }
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        
        if !meta.published_channels.iter().any(|c| c == &channel) {
            meta.published_channels.push(channel);
        }
        
        let out = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(p, out)?;
        Ok(())
    }

    /// Атомарно обновляет данные канала (суммаризацию, пост и статус публикации)
    async fn update_channel_data(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        summary_text: Option<&str>,
        post_text: Option<&str>,
        is_published: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            match serde_json::from_str::<CacheMetadata>(&data) {
                Ok(parsed_meta) => parsed_meta,
                Err(e) => {
                    tracing::warn!(project_id = %project_id, error = %e, "failed to parse existing metadata.json, creating new one");
                    CacheMetadata {
                        project_id: project_id.to_string().into(),
                        docx_path: String::new().into(),
                        markdown_path: String::new().into(),
                        published_channels: vec![],
                        created_at: chrono::Utc::now().to_rfc3339().into(),
                        channel_summaries: std::collections::HashMap::new(),
                        channel_posts: std::collections::HashMap::new(),
                        crawl_metadata: vec![],
                    }
                }
            }
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        
        // Обновляем суммаризацию, если передана
        if let Some(summary) = summary_text {
            meta.channel_summaries.insert(channel, summary.to_string().into());
        }
        
        // Обновляем пост, если передан
        if let Some(post) = post_text {
            meta.channel_posts.insert(channel, post.to_string().into());
        }
        
        // Обновляем статус публикации
        if is_published && !meta.published_channels.iter().any(|c| c == &channel) {
            meta.published_channels.push(channel);
        }
        
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&p, json)?;
        Ok(())
    }

    async fn has_data(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // new layout first
        let p = self.project_dir(project_id).join("extracted.md");
        if p.exists() {
            return Ok(true);
        }
        // legacy fallback
        let legacy = Path::new(&self.cache_dir).join(format!("{}_extracted.md", project_id));
        Ok(legacy.exists())
    }

    async fn has_summary(&self, project_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        if let Some(meta) = meta {
            // Проверяем, есть ли суммаризации в каналах
            if !meta.channel_summaries.is_empty() {
                return Ok(true);
            }
        }
        
        // Legacy fallback - проверяем старый файл summary.txt
        let legacy = Path::new(&self.cache_dir).join(format!("{}_summary.txt", project_id));
        Ok(legacy.exists())
    }

    async fn is_published_in_channel(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.published_channels.contains(&channel)).unwrap_or(false))
    }

    async fn get_published_channels(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.published_channels.iter().map(|c| c.as_str().to_string()).collect()).unwrap_or_default())
    }

    async fn has_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.channel_summaries.contains_key(&channel)).unwrap_or(false))
    }

    async fn load_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<Option<SummaryText>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.and_then(|m| m.channel_summaries.get(&channel).cloned()))
    }

    async fn update_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        summary_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or(CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        
        meta.channel_summaries.insert(channel, summary_text.to_string().into());
        
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&p, json)?;
        Ok(())
    }

    async fn has_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.channel_posts.contains_key(&channel)).unwrap_or(false))
    }

    async fn load_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
    ) -> Result<Option<PostText>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.and_then(|m| m.channel_posts.get(&channel).cloned()))
    }

    async fn update_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        post_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or(CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        
        meta.channel_posts.insert(channel, post_text.to_string().into());
        
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&p, json)?;
        Ok(())
    }

    async fn load_manifest(&self) -> Result<crate::models::types::Manifest, Box<dyn std::error::Error + Send + Sync>> {
        let manifest_path = Path::new(&self.cache_dir).join("manifest.json");
        if manifest_path.exists() {
            if let Ok(s) = fs::read_to_string(&manifest_path) {
                if let Ok(m) = serde_json::from_str::<crate::models::types::Manifest>(&s) {
                    return Ok(m);
                }
            }
        }
        Ok(crate::models::types::Manifest::default())
    }

    async fn save_manifest(&self, manifest: &crate::models::types::Manifest) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Ensure cache dir exists
        let manifest_path = Path::new(&self.cache_dir).join("manifest.json");
        if let Some(dir) = manifest_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(manifest).unwrap_or_else(|_| "{}".to_string());
        tracing::info!(manifest_path = %manifest_path.display(), manifest_content = %json, "npalist: saving manifest");
        fs::write(&manifest_path, json)?;
        Ok(())
    }

    async fn update_min_published_project_id(&self, min_id: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut manifest = self.load_manifest().await?;
        manifest.min_published_project_id = Some(min_id);
        tracing::info!(new_min_id = min_id, "cache_manager: updating min_published_project_id");
        self.save_manifest(&manifest).await?;
        Ok(())
    }

    async fn update_all_channels_data(
        &self,
        project_id: &str,
        channel_data: &[(crate::models::channel::PublisherChannel, &str, &str)],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or(CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
                crawl_metadata: vec![],
            }
        };
        
        // Обновляем данные для всех каналов
        for (channel, summary, post) in channel_data {
            meta.channel_summaries.insert(*channel, summary.to_string().into());
            meta.channel_posts.insert(*channel, post.to_string().into());
            
            // Добавляем канал в published_channels, если его там нет
            if !meta.published_channels.iter().any(|c| c == channel) {
                meta.published_channels.push(*channel);
            }
        }
        
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&p, json)?;
        Ok(())
    }

    async fn is_fully_published(&self, project_id: &str, enabled_channels: &[crate::models::channel::PublisherChannel]) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Загружаем метаданные
        let metadata = match self.load_metadata(project_id).await? {
            Some(meta) => meta,
            None => return Ok(false), // Нет метаданных - не опубликован
        };

        // Проверяем, что элемент опубликован во все включенные каналы
        for channel in enabled_channels {
            if !metadata.published_channels.contains(channel) {
                tracing::info!(
                    project_id = project_id,
                    missing_channel = %channel,
                    "Element not fully published - missing channel"
                );
                return Ok(false);
            }
        }

        tracing::info!(
            project_id = project_id,
            published_channels = ?metadata.published_channels,
            enabled_channels = ?enabled_channels,
            "Element is fully published in all enabled channels"
        );
        Ok(true)
    }
}
