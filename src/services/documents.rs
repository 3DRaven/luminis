//

use crate::crawlers::FileIdScanner;
use crate::traits::markdown_fetcher::MarkdownFetcher;
use markdownify::docx;
use reqwest::Client;
use std::io::Write;
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
