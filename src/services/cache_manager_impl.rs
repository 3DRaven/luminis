use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::fs;
use serde_json;
use bon::Builder;

use crate::traits::cache_manager::CacheManager;
use crate::services::documents::CacheMetadata;

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
        published_channels: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base = self.project_dir(project_id);
        fs::create_dir_all(&base)?;
        let ts = chrono::Utc::now().to_rfc3339();

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
            project_id: project_id.to_string(),
            docx_path: docx_path.to_string_lossy().to_string(),
            markdown_path: md_path.to_string_lossy().to_string(),
            summary_path: if !summary_text.is_empty() {
                Some(sum_path.to_string_lossy().to_string())
            } else {
                None
            },
            post_path: if !post_text.is_empty() {
                Some(post_path.to_string_lossy().to_string())
            } else {
                None
            },
            published_channels: published_channels.to_vec(),
            created_at: ts,
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
        new_channels: &[&str],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let p = self.meta_path_for(project_id);
        let mut meta = if p.exists() {
            let data = fs::read_to_string(&p)?;
            serde_json::from_str::<CacheMetadata>(&data).unwrap_or(CacheMetadata {
                project_id: project_id.to_string(),
                docx_path: String::new(),
                markdown_path: String::new(),
                summary_path: None,
                post_path: None,
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            })
        } else {
            CacheMetadata {
                project_id: project_id.to_string(),
                docx_path: String::new(),
                markdown_path: String::new(),
                summary_path: None,
                post_path: None,
                published_channels: vec![],
                created_at: chrono::Utc::now().to_rfc3339(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            }
        };
        for ch in new_channels {
            if !meta.published_channels.iter().any(|c| c == ch) {
                meta.published_channels.push(ch.to_string());
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
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let channels = self.get_published_channels(project_id).await?;
        Ok(channels.iter().any(|c| c == channel))
    }

    async fn get_published_channels(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.published_channels).unwrap_or_default())
    }

    async fn has_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.channel_summaries.contains_key(channel)).unwrap_or(false))
    }

    async fn load_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.and_then(|m| m.channel_summaries.get(channel).cloned()))
    }

    async fn save_channel_summary(
        &self,
        project_id: &str,
        channel: &str,
        summary_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut meta = self.load_metadata(project_id).await?
            .unwrap_or_else(|| CacheMetadata {
                project_id: project_id.to_string(),
                docx_path: String::new(),
                markdown_path: String::new(),
                summary_path: None,
                post_path: None,
                published_channels: Vec::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            });
        
        meta.channel_summaries.insert(channel.to_string(), summary_text.to_string());
        
        let meta_path = self.meta_path_for(project_id);
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&meta_path, json)?;
        Ok(())
    }

    async fn has_channel_post(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.map(|m| m.channel_posts.contains_key(channel)).unwrap_or(false))
    }

    async fn load_channel_post(
        &self,
        project_id: &str,
        channel: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.load_metadata(project_id).await?;
        Ok(meta.and_then(|m| m.channel_posts.get(channel).cloned()))
    }

    async fn save_channel_post(
        &self,
        project_id: &str,
        channel: &str,
        post_text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut meta = self.load_metadata(project_id).await?
            .unwrap_or_else(|| CacheMetadata {
                project_id: project_id.to_string(),
                docx_path: String::new(),
                markdown_path: String::new(),
                summary_path: None,
                post_path: None,
                published_channels: Vec::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                channel_summaries: std::collections::HashMap::new(),
                channel_posts: std::collections::HashMap::new(),
            });
        
        meta.channel_posts.insert(channel.to_string(), post_text.to_string());
        
        let meta_path = self.meta_path_for(project_id);
        let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
        fs::write(&meta_path, json)?;
        Ok(())
    }
}
