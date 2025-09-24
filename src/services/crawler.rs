use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::traits::cache_manager::CacheManager;
use crate::traits::crawler::Crawler;
use async_trait::async_trait;
use bon::{Builder, bon};
use regex::Regex;
use reqwest::Client;
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use tracing::info;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Manifest {
    #[serde(default)]
    sources: HashMap<String, SourceState>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
struct SourceState {
    #[serde(default)]
    last_offset: u32,
    #[serde(default)]
    last_limit: u32,
}

// (no secrets stored here)

impl Manifest {
    pub fn path() -> &'static str {
        "./cache/manifest.json"
    }

    pub fn load() -> Manifest {
        let p = Path::new(Self::path());
        if p.exists() {
            if let Ok(s) = fs::read_to_string(p) {
                if let Ok(m) = serde_json::from_str::<Manifest>(&s) {
                    return m;
                }
            }
        }
        Manifest::default()
    }

    pub fn save(&self) {
        // Ensure cache dir exists
        if let Some(dir) = Path::new(Self::path()).parent() {
            let _ = fs::create_dir_all(dir);
        }
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        let _ = fs::write(Self::path(), json);
    }
}

/// RSS crawler: парсит XML и извлекает CrawlItem с метаданными из description
pub struct RssCrawler {
    client: Client,
    url: String,
    regex: Regex,
}

#[bon]
impl RssCrawler {
    #[builder]
    pub fn new(
        url: String,
        regex: Regex,
        timeout: Duration,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self { client, url, regex })
    }
}

#[async_trait]
impl Crawler for RssCrawler {
    async fn fetch(&self) -> Result<Vec<CrawlItem>, Box<dyn std::error::Error + Send + Sync>> {
        let rss_url = &self.url;
        info!(url = %rss_url, "rss: fetch");
        let re = &self.regex;
        let xml = self.client.get(rss_url).send().await?.text().await?;
        let doc = Document::parse(&xml)?;
        let mut items: Vec<CrawlItem> = Vec::new();
        let default_link_re = Regex::new(r#"projects/(\d{5,})"#).unwrap();
        for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
            let text_of = |name: &str| -> Option<String> {
                item.children()
                    .find(|n| n.has_tag_name(name))
                    .and_then(|n| n.text())
                    .map(|s| s.trim().to_string())
            };
            let guid = text_of("guid");
            let link = text_of("link");
            let title = text_of("title");
            let desc = text_of("description");
            let author = text_of("author");
            // Try to extract project_id in order: guid -> link -> description, using provided regex.
            let mut pid: Option<String> = None;
            if let Some(g) = guid.as_deref() {
                pid = re
                    .captures(g)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());
            }
            if pid.is_none() {
                if let Some(l) = link.as_deref() {
                    pid = re
                        .captures(l)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string());
                    if pid.is_none() {
                        pid = default_link_re
                            .captures(l)
                            .and_then(|c| c.get(1))
                            .map(|m| m.as_str().to_string());
                    }
                }
            }
            if pid.is_none() {
                if let Some(d) = desc.as_deref() {
                    pid = re
                        .captures(d)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string());
                }
            }
            if let Some(project_id) = pid {
                let (status, stage, kind, department, responsible, publish_date) =
                    parse_rss_description(desc.as_deref());
                let url = link.clone().unwrap_or_default();
                let title_val = title.clone().unwrap_or_default();
                if url.is_empty() || title_val.is_empty() {
                    continue;
                }
                let mut metadata: Vec<MetadataItem> = Vec::new();
                if let Some(v) = status {
                    metadata.push(MetadataItem::Status(v));
                }
                if let Some(v) = stage {
                    metadata.push(MetadataItem::Stage(v));
                }
                if let Some(v) = kind {
                    metadata.push(MetadataItem::Kind(v));
                }
                if let Some(v) = department {
                    metadata.push(MetadataItem::Department(v));
                }
                if let Some(v) = responsible {
                    metadata.push(MetadataItem::Responsible(v));
                }
                if let Some(v) = publish_date {
                    metadata.push(MetadataItem::PublishDate(v));
                }
                if let Some(v) = author {
                    metadata.push(MetadataItem::Author(v));
                }
                items.push(CrawlItem {
                    title: title_val,
                    url,
                    body: String::new(),
                    project_id: Some(project_id),
                    metadata,
                });
            }
        }
        Ok(items)
    }
}

/// Crawler для API списка НПА с пагинацией, состояние в manifest.json
pub struct NpaListCrawler {
    client: Client,
    url_template: String,
    limit: u32,
    project_id_re: Option<Regex>,
    cache_manager: Arc<dyn CacheManager>,
    poll_delay: Duration,
}

#[bon]
impl NpaListCrawler {
    #[builder]
    pub fn new(
        url_template: String,
        limit_opt: Option<u32>,
        project_id_re: Option<Regex>,
        timeout: Duration,
        cache_manager: Arc<dyn CacheManager>,
        poll_delay: Duration,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            url_template,
            limit: limit_opt.unwrap_or(50),
            project_id_re,
            cache_manager,
            poll_delay,
        })
    }
}

#[async_trait]
impl Crawler for NpaListCrawler {
    async fn fetch(&self) -> Result<Vec<CrawlItem>, Box<dyn std::error::Error + Send + Sync>> {
        let mut manifest = Manifest::load();
        let key = self.url_template.to_string();
        let limit = self.limit;
        let state = manifest.sources.get(&key).cloned().unwrap_or_default();
        let last_offset = state.last_offset;

        // Always fetch latest page (offset=0) first
        let url_latest = self
            .url_template
            .replace("{limit}", &limit.to_string())
            .replace("{offset}", &0.to_string());
        info!(%url_latest, "npalist: fetch latest page (offset=0)");
        let latest_projects = self.client.get(&url_latest).send().await?;
        if !latest_projects.status().is_success() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "npalist: http error on latest: {}",
                    latest_projects.status()
                ),
            )));
        }
        let latest_text = latest_projects.text().await?;
        let latest = parse_npa_projects(&latest_text, self.project_id_re.as_ref());

        // Filter out already cached items (by presence of extracted.md)
        let mut latest_not_cached: Vec<CrawlItem> = Vec::new();
        for it in latest.into_iter() {
            let cached = if let Some(pid) = it.project_id.as_deref() {
                self.cache_manager.has_data(pid).await.unwrap_or(false)
            } else {
                false
            };
            if !cached {
                latest_not_cached.push(it);
            }
        }

        // If new, not-cached items found in latest, return them immediately
        if !latest_not_cached.is_empty() {
            info!(
                count = latest_not_cached.len(),
                "npalist: latest page has not-cached items"
            );
            return Ok(latest_not_cached);
        }

        // If no new items found in latest, go deeper into history using last_offset
        let mut current_offset = if last_offset > 0 { last_offset } else { limit };
        let mut aggregated: Vec<CrawlItem> = Vec::new();
        loop {
            let url_cont = self
                .url_template
                .replace("{limit}", &limit.to_string())
                .replace("{offset}", &current_offset.to_string());
            info!(%url_cont, current_offset, "npalist: deep dive into history");

            let history_page = self.client.get(&url_cont).send().await?;
            if !history_page.status().is_success() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("npalist: http error on history: {}", history_page.status()),
                )));
            }
            let history_page_text = history_page.text().await?;
            let history_projects =
                parse_npa_projects(&history_page_text, self.project_id_re.as_ref());

            if history_projects.is_empty() {
                return Ok(aggregated);
            }

            // Filter out cached
            let mut history_projects_not_cached: Vec<CrawlItem> = Vec::new();
            for it in history_projects.into_iter() {
                let cached = if let Some(pid) = it.project_id.as_deref() {
                    self.cache_manager.has_data(pid).await.unwrap_or(false)
                } else {
                    false
                };
                if !cached {
                    history_projects_not_cached.push(it);
                }
            }

            // Update offset after real attempt to read
            let entry = manifest.sources.entry(key.clone()).or_default();
            entry.last_limit = limit;
            entry.last_offset = current_offset.saturating_add(limit);
            let new_offset = entry.last_offset;
            manifest.save();
            info!(new_offset, "npalist: updated offset after reading history");

            aggregated.extend(history_projects_not_cached);

            current_offset = new_offset;
            if self.poll_delay.as_millis() > 0 {
                info!(
                    delay_ms = self.poll_delay.as_millis(),
                    current_offset,
                    "npalist: sleeping before next history page request to avoid rate limiting"
                );
                tokio::time::sleep(self.poll_delay).await;
            }
        }
    }
}

fn parse_npa_projects(text: &str, project_id_re: Option<&Regex>) -> Vec<CrawlItem> {
    let mut out = Vec::new();
    let doc = Document::parse(text).unwrap_or_else(|_| Document::parse("<projects/>").unwrap());
    for proj in doc.descendants().filter(|n| n.has_tag_name("project")) {
        let mut project_attr_id = proj.attribute("id").unwrap_or("").to_string();
        if project_attr_id.is_empty() {
            continue;
        }
        let text_of = |name: &str| -> Option<String> {
            proj.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
                .map(|s| s.trim().to_string())
        };
        let text_and_id = |name: &str| -> (Option<String>, Option<String>) {
            if let Some(node) = proj.children().find(|n| n.has_tag_name(name)) {
                (
                    node.text()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                    node.attribute("id").map(|v| v.to_string()),
                )
            } else {
                (None, None)
            }
        };
        let title_opt = text_of("title");
        let pid_text = text_of("projectId");
        let title = match (title_opt.clone(), pid_text.clone()) {
            (Some(t), _) => t,
            (None, Some(pid)) => pid,
            (None, None) => return Vec::new(),
        };
        let mut url = format!("https://regulation.gov.ru/projects/{}", project_attr_id);
        if let Some(re) = project_id_re {
            // Проверяем соответствие по regex: пытаемся извлечь id из project_attr_id
            if let Some(cap) = re.captures(&project_attr_id).and_then(|c| c.get(1)) {
                project_attr_id = cap.as_str().to_string();
                url = format!("https://regulation.gov.ru/projects/{}", project_attr_id);
            } else {
                // Если regex не подтверждает id, пропускаем запись
                continue;
            }
        }
        let (stage_text, stage_id) = text_and_id("stage");
        let (status_text, status_id) = text_and_id("status");
        let (ri_text, ri_id) = text_and_id("regulatoryImpact");
        let (pr_text, pr_id) = text_and_id("procedureResult");
        let (kind_text, kind_id) = text_and_id("kind");
        let (dept_text, dept_id) = text_and_id("department");
        let (proc_text, proc_id) = text_and_id("procedure");
        let parallel_files: Vec<String> = proj
            .children()
            .filter(|n| n.has_tag_name("parallelStageFile"))
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
            .collect();

        let mut body_lines: Vec<String> = Vec::new();
        if let Some(d) = text_of("date") {
            body_lines.push(format!("Дата: {}", d));
        }
        if let Some(pd) = text_of("publishDate") {
            body_lines.push(format!("Публикация: {}", pd));
        }
        if let Some(s) = &stage_text {
            body_lines.push(format!(
                "Стадия: {}{}",
                s,
                stage_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &status_text {
            body_lines.push(format!(
                "Статус: {}{}",
                s,
                status_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &ri_text {
            body_lines.push(format!(
                "Рег. влияние: {}{}",
                s,
                ri_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &pr_text {
            body_lines.push(format!(
                "Результат процедуры: {}{}",
                s,
                pr_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &kind_text {
            body_lines.push(format!(
                "Вид: {}{}",
                s,
                kind_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &dept_text {
            body_lines.push(format!(
                "Ведомство: {}{}",
                s,
                dept_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }
        if let Some(s) = &proc_text {
            body_lines.push(format!(
                "Процедура: {}{}",
                s,
                proc_id
                    .as_ref()
                    .map(|v| format!(" (id: {})", v))
                    .unwrap_or_default()
            ));
        }

        let body = if body_lines.is_empty() {
            String::new()
        } else {
            format!("{}\n{}", title, body_lines.join("\n"))
        };
        let mut metadata: Vec<MetadataItem> = Vec::new();
        if let Some(v) = text_of("date") {
            metadata.push(MetadataItem::Date(v));
        }
        if let Some(v) = text_of("publishDate") {
            metadata.push(MetadataItem::PublishDate(v));
        }
        if let Some(v) = stage_text {
            metadata.push(MetadataItem::Stage(v));
        }
        if let Some(v) = stage_id {
            metadata.push(MetadataItem::StageId(v));
        }
        if let Some(v) = status_text {
            metadata.push(MetadataItem::Status(v));
        }
        if let Some(v) = status_id {
            metadata.push(MetadataItem::StatusId(v));
        }
        if let Some(v) = ri_text {
            metadata.push(MetadataItem::RegulatoryImpact(v));
        }
        if let Some(v) = ri_id {
            metadata.push(MetadataItem::RegulatoryImpactId(v));
        }
        if let Some(v) = pr_text {
            metadata.push(MetadataItem::ProcedureResult(v));
        }
        if let Some(v) = pr_id {
            metadata.push(MetadataItem::ProcedureResultId(v));
        }
        if let Some(v) = kind_text {
            metadata.push(MetadataItem::Kind(v));
        }
        if let Some(v) = kind_id {
            metadata.push(MetadataItem::KindId(v));
        }
        if let Some(v) = dept_text {
            metadata.push(MetadataItem::Department(v));
        }
        if let Some(v) = dept_id {
            metadata.push(MetadataItem::DepartmentId(v));
        }
        if let Some(v) = proc_text {
            metadata.push(MetadataItem::Procedure(v));
        }
        if let Some(v) = proc_id {
            metadata.push(MetadataItem::ProcedureId(v));
        }
        if let Some(v) = text_of("responsible") {
            metadata.push(MetadataItem::Responsible(v));
        }
        if !parallel_files.is_empty() {
            metadata.push(MetadataItem::ParallelStageFiles(parallel_files));
        }

        out.push(CrawlItem {
            title,
            url,
            body,
            project_id: Some(project_attr_id.clone()),
            metadata,
        });
    }
    out
}

fn parse_rss_description(
    desc: Option<&str>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let status = None;
    let stage = None;
    let mut kind = None;
    let department = None;
    let mut responsible = None;
    let mut publish_date = None;
    if let Some(d) = desc {
        for line in d.lines() {
            let line = line.trim();
            if line.starts_with("Вид:") {
                kind = Some(
                    line.trim_start_matches("Вид:")
                        .trim()
                        .trim_matches('"')
                        .to_string(),
                );
            }
            if line.starts_with("Процедура:") { /* could map to procedure if needed */ }
            if line.starts_with("Разработчик:") {
                responsible = Some(
                    line.trim_start_matches("Разработчик:")
                        .trim()
                        .trim_matches('"')
                        .to_string(),
                );
            }
            if line.starts_with("Дата создания:") {
                publish_date = Some(line.trim_start_matches("Дата создания:").trim().to_string());
            }
        }
    }
    (status, stage, kind, department, responsible, publish_date)
}

/// Scanner for stages endpoint: extracts fileId and may enrich metadata later
#[derive(Builder)]
pub struct FileIdScanner {
    #[builder(default)]
    client: Client,
}

impl FileIdScanner {
    pub async fn fetch_file_id(
        &self,
        url: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        info!(%url, "fileid: fetch");
        let response = self.client.get(url).send().await?;
        info!(status = %response.status(), "fileid: response status");
        let body = response.text().await?;
        info!(body_len = body.len(), "fileid: response body length");
        let re = Regex::new(r#"fileId"\s*:\s*"([^"]+)"#).unwrap();
        for caps in re.captures_iter(&body) {
            if let Some(m) = caps.get(1) {
                let file_id = m.as_str().to_string();
                info!(%file_id, "fileid: found fileId");
                return Ok(Some(file_id));
            }
        }
        info!("fileid: no fileId found in response");
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub struct CrawlItem {
    pub title: String,
    pub url: String,
    pub body: String,
    pub project_id: Option<String>,
    pub metadata: Vec<MetadataItem>,
}

#[derive(Clone, Debug, Display)]
#[strum(serialize_all = "snake_case")]
pub enum MetadataItem {
    Date(String),
    PublishDate(String),
    RegulatoryImpact(String),
    RegulatoryImpactId(String),
    Responsible(String),
    Author(String),
    Department(String),
    DepartmentId(String),
    Status(String),
    StatusId(String),
    Stage(String),
    StageId(String),
    Kind(String),
    KindId(String),
    Procedure(String),
    ProcedureId(String),
    ProcedureResult(String),
    ProcedureResultId(String),
    NextStageDuration(String),
    ParallelStageStartDiscussion(String),
    ParallelStageEndDiscussion(String),
    StartDiscussion(String),
    EndDiscussion(String),
    Problem(String),
    Objectives(String),
    CirclePersons(String),
    SocialRelations(String),
    Rationale(String),
    TransitionPeriod(String),
    PlanDate(String),
    CompliteDateAct(String),
    CompliteNumberDepAct(String),
    CompliteNumberRegAct(String),
    ParallelStageFiles(Vec<String>),
}

// Old crawler trait and structs removed in favor of specialized scanners
