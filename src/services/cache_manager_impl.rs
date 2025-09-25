use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::fs;
use serde_json;
use bon::Builder;

use crate::traits::cache_manager::CacheManager;
use crate::services::documents::CacheMetadata;
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
        summary_text: &str,
        post_text: &str,
        published_channels: &[PublisherChannel],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base = self.project_dir(project_id);
        fs::create_dir_all(&base)?;
        let ts: CreatedAt = chrono::Utc::now().to_rfc3339().into();

        // per-project subdir layout
        let docx_path = base.join("source.docx");
        let md_path = base.join("extracted.md");
        let sum_path = base.join("summary.txt");
        let post_path = base.join("post.txt");
        let meta_path = base.join("metadata.json");

        if let Some(bytes) = docx_bytes {
            fs::write(&docx_path, bytes)?;
        }
        fs::write(&md_path, markdown_text)?;
        if !summary_text.is_empty() {
            fs::write(&sum_path, summary_text)?;
        }
        if !post_text.is_empty() {
            fs::write(&post_path, post_text)?;
        }

        let meta = CacheMetadata {
            project_id: project_id.to_string().into(),
            docx_path: docx_path.to_string_lossy().to_string().into(),
            markdown_path: md_path.to_string_lossy().to_string().into(),
            summary_path: if !summary_text.is_empty() {
                Some(sum_path.to_string_lossy().to_string().into())
            } else {
                None
            },
            post_path: if !post_text.is_empty() {
                Some(post_path.to_string_lossy().to_string().into())
            } else {
                None
            },
            published_channels: published_channels.to_vec(),
            created_at: ts.into(),
            channel_summaries: std::collections::HashMap::new(),
            channel_posts: std::collections::HashMap::new(),
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
        // new layout first
        let p = self.project_dir(project_id).join("summary.txt");
        let s = if p.exists() {
            fs::read_to_string(p)?
        } else {
            // legacy fallback
            let legacy = Path::new(&self.cache_dir).join(format!("{}_summary.txt", project_id));
            if !legacy.exists() {
                return Ok(None);
            }
            fs::read_to_string(legacy)?
        };
        Ok(Some(s))
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
                summary_path: None,
                post_path: None,
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                summary_path: None,
                post_path: None,
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
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
        Ok(meta.and_then(|m| m.summary_path).is_some())
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

    async fn save_channel_summary(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        summary_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut meta = self.load_metadata(project_id).await?
            .unwrap_or_else(|| CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                summary_path: None,
                post_path: None,
                published_channels: Vec::new(),
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            });
        
        meta.channel_summaries.insert(channel, summary_text.to_string().into());
        
        let meta_path = self.meta_path_for(project_id);
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&meta_path, json)?;
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

    async fn save_channel_post(
        &self,
        project_id: &str,
        channel: PublisherChannel,
        post_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut meta = self.load_metadata(project_id).await?
            .unwrap_or_else(|| CacheMetadata {
                project_id: project_id.to_string().into(),
                docx_path: String::new().into(),
                markdown_path: String::new().into(),
                summary_path: None,
                post_path: None,
                published_channels: Vec::new(),
                created_at: chrono::Utc::now().to_rfc3339().into(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            });
        
        meta.channel_posts.insert(channel, post_text.to_string().into());
        
        let meta_path = self.meta_path_for(project_id);
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&meta_path, json)?;
        Ok(())
    }
}
