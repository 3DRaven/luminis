//

use crate::services::crawler::FileIdScanner;
use crate::traits::markdown_fetcher::MarkdownFetcher;
use markdownify::docx;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{debug, info};
use bon::bon;

/// Реализация MarkdownFetcher, получающая DOCX и извлекающая из него markdown
pub struct DocxMarkdownFetcher {
    client: Client,
    file_id_url_template: Option<String>,
    files_base_url: Option<String>,
}

#[bon]
impl DocxMarkdownFetcher {
    #[builder]
    pub fn new(file_id_url_template: Option<String>) -> Self {
        // Derive files base URL from file_id template host if provided
        let files_base_url = file_id_url_template.as_ref().and_then(|tpl| {
            let to_parse = tpl.replace("{project_id}", "0");
            url::Url::parse(&to_parse)
                .ok()
                .map(|u| {
                    let host = u.host_str().unwrap_or("localhost");
                    match u.port() {
                        Some(port) => format!("{}://{}:{}", u.scheme(), host, port),
                        None => format!("{}://{}", u.scheme(), host),
                    }
                })
        });
        Self {
            client: Client::new(),
            file_id_url_template,
            files_base_url,
        }
    }

    /// Внутренняя реализация получения DOCX и извлечения markdown
    async fn fetch_docx_internal(
        &self,
        project_id: &str,
    ) -> Result<Option<(Vec<u8>, String)>, Box<dyn std::error::Error + Send + Sync>> {
        info!(%project_id, "docx: get fileId");
        // Resolve fileId using configured template
        let tpl = self.file_id_url_template.as_ref().ok_or_else(||
            Box::<dyn std::error::Error + Send + Sync>::from("crawler.file_id.url is required in config (no fallback stages endpoint)")
        )?;
        let url = tpl.replace("{project_id}", project_id);
        let scanner = FileIdScanner::builder().client(Client::new()).build();
        let file_id = scanner.fetch_file_id(&url).await?;
        let file_id = match file_id {
            Some(v) => v,
            None => {
                info!(%project_id, "docx: skip project without fileId");
                return Ok(None);
            }
        };
        info!(%file_id, "docx: downloading file");
        let base = self
            .files_base_url
            .as_deref()
            .unwrap_or("https://regulation.gov.ru");
        let file_url = format!("{}/api/public/Files/GetFile?fileId={}", base, file_id);
        info!(url = %file_url, "docx: GET file url");
        let response = self.client.get(&file_url).send().await?;
        info!(status = %response.status(), "docx: response status");
        let bytes = response.bytes().await?;
        info!(size = bytes.len(), "docx: downloaded");

        // Проверяем на пустой файл
        if bytes.is_empty() {
            info!(%project_id, "docx: file is empty, skipping");
            return Ok(None);
        }

        let text = Self::extract_markdown_from_docx(bytes.as_ref())?;
        debug!(len = text.len(), "docx: extracted markdown");
        Ok(Some((bytes.to_vec(), text)))
    }

    // kept functions below
}

#[derive(Serialize, Deserialize)]
pub struct CacheMetadata {
    pub project_id: String,
    pub docx_path: String,
    pub markdown_path: String,
    pub summary_path: Option<String>,
    pub post_path: Option<String>,
    pub published_channels: Vec<String>,
    pub created_at: String,
    // Новые поля для суммаризаций по каналам
    pub channel_summaries: std::collections::HashMap<String, String>, // channel_name -> summary_text
    pub channel_posts: std::collections::HashMap<String, String>,     // channel_name -> post_text
}

pub fn save_cache_artifacts(
    cache_dir: &str,
    project_id: &str,
    docx_bytes: Option<&[u8]>,
    markdown_text: &str,
    summary_text: &str,
    post_text: &str,
    published_channels: &[String],
) -> std::io::Result<()> {
    let base = project_dir(cache_dir, project_id);
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
        channel_summaries: HashMap::new(), // Будет заполняться отдельно
        channel_posts: HashMap::new(),      // Будет заполняться отдельно
    };
    let json = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
    fs::write(&meta_path, json)?;
    Ok(())
}

fn meta_path_for(cache_dir: &str, project_id: &str) -> PathBuf {
    project_dir(cache_dir, project_id).join("metadata.json")
}

pub fn load_cache_metadata(cache_dir: &str, project_id: &str) -> io::Result<Option<CacheMetadata>> {
    // new layout first
    let p = meta_path_for(cache_dir, project_id);
    let data = if p.exists() {
        fs::read_to_string(p)?
    } else {
        // legacy fallback
        let legacy = Path::new(cache_dir).join(format!("{}_metadata.json", project_id));
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

pub fn load_cached_summary(cache_dir: &str, project_id: &str) -> io::Result<Option<String>> {
    // new layout first
    let p = project_dir(cache_dir, project_id).join("summary.txt");
    let s = if p.exists() {
        fs::read_to_string(p)?
    } else {
        // legacy fallback
        let legacy = Path::new(cache_dir).join(format!("{}_summary.txt", project_id));
        if !legacy.exists() {
            return Ok(None);
        }
        fs::read_to_string(legacy)?
    };
    Ok(Some(s))
}

pub fn add_published_channels(
    cache_dir: &str,
    project_id: &str,
    new_channels: &[&str],
) -> io::Result<()> {
    let p = meta_path_for(cache_dir, project_id);
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
            channel_summaries: HashMap::new(),
            channel_posts: HashMap::new(),
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
            channel_summaries: HashMap::new(),
            channel_posts: HashMap::new(),
        }
    };
    for ch in new_channels {
        if !meta.published_channels.iter().any(|c| c == ch) {
            meta.published_channels.push(ch.to_string());
        }
    }
    let out = serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string());
    fs::write(p, out)
}

fn project_dir(cache_dir: &str, project_id: &str) -> PathBuf {
    let mut p = PathBuf::from(cache_dir);
    p.push(project_id);
    p
}

// New helper that converts DOCX bytes to Markdown via markdownify
impl DocxMarkdownFetcher {
    fn extract_markdown_from_docx(
        docx_bytes: &[u8],
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!(bytes_len = docx_bytes.len(), "docx: received bytes for markdownify");
        let mut tmp = tempfile::NamedTempFile::new()?;
        tmp.write_all(docx_bytes)?;
        let md =
            docx::docx_convert(tmp.path()).map_err(|e| format!("markdownify failed: {}", e))?;
        info!(len = md.len(), "docx: extracted markdown");
        Ok(md)
    }
}

#[async_trait::async_trait]
impl MarkdownFetcher for DocxMarkdownFetcher {
    async fn fetch_markdown(
        &self,
        project_id: &str,
    ) -> Result<Option<(Vec<u8>, String)>, Box<dyn std::error::Error + Send + Sync>> {
        self.fetch_docx_internal(project_id).await
    }
}
