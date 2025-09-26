use std::sync::Arc;
use std::time::Duration;

use crate::traits::cache_manager::CacheManager;
use crate::traits::crawler::Crawler;
use crate::models::channel::PublisherChannel;
use crate::models::types::{CrawlItem, MetadataItem};
use async_trait::async_trait;
use bon::{Builder, bon};
use regex::Regex;
use reqwest::Client;
use roxmltree::Document;
use tracing::{info, error};
use tokio::sync::mpsc;

/// Crawler для API списка НПА с пагинацией, состояние в manifest.json
pub struct NpaListCrawler {
    client: Client,
    url_template: String,
    limit: u32,
    project_id_re: Option<Regex>,
    cache_manager: Arc<dyn CacheManager>,
    poll_delay: Duration,
    enabled_channels: Vec<PublisherChannel>,
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
        enabled_channels: Vec<PublisherChannel>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            url_template,
            limit: limit_opt.unwrap_or(50),
            project_id_re,
            cache_manager,
            poll_delay,
            enabled_channels,
        })
    }
}

#[async_trait]
impl Crawler for NpaListCrawler {
    async fn fetch_stream(&self, sender: mpsc::Sender<CrawlItem>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let manifest = self.cache_manager.load_manifest().await?;
        let limit = self.limit;
        let min_published_project_id = manifest.min_published_project_id;
        
        info!(min_published_project_id = min_published_project_id, "npalist: loaded manifest state for streaming");

        // 1. Всегда читаем offset=0 (новые записи)
        let url_latest = self
            .url_template
            .replace("{limit}", &limit.to_string())
            .replace("{offset}", &0.to_string());
        info!(%url_latest, "npalist: fetch latest page (offset=0) for streaming");
        
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
        let total_items = latest.len();
        
        info!(total_items = total_items, "npalist: parsing latest projects for streaming");
        
        // Отправляем элементы по одному, если они не полностью опубликованы
        let mut latest_not_published: Vec<CrawlItem> = Vec::new();
        let mut current_max_id: Option<u32> = None;
        let mut current_min_id: Option<u32> = None;
        
        for it in latest.into_iter() {
            if let Some(pid) = it.project_id.as_deref() {
                if let Ok(pid_num) = pid.parse::<u32>() {
                    // Проверяем, полностью ли опубликован элемент
                    let fully_published = self.cache_manager.is_fully_published(pid, &self.enabled_channels).await?;
                    // Обновляем min/max ID
                    current_max_id = Some(current_max_id.map_or(pid_num, |max| max.max(pid_num)));
                    current_min_id = Some(current_min_id.map_or(pid_num, |min| min.min(pid_num)));
                    
                    if fully_published {
                        info!(project_id = pid_num, "npalist: project is fully published, skipping");
                    } else {
                        info!(project_id = pid_num, "npalist: project not fully published, sending to worker");
                        // Сначала добавляем в список, потом отправляем
                        latest_not_published.push(it.clone());
                        // Отправляем элемент в канал (может зависнуть если канал полон)
                        if let Err(_) = sender.send(it).await {
                            info!("npalist: worker channel closed, stopping streaming");
                            break;
                        }
                    }
                }
            }
        }

        info!(
            latest_not_published_count = latest_not_published.len(),
            current_min_id = ?current_min_id,
            current_max_id = ?current_max_id,
            "npalist: finished processing latest items"
        );

        // Обновляем min_published_project_id в manifest после обработки элементов
        if let Some(current_min_id) = current_min_id {
            self.cache_manager.update_min_published_project_id(current_min_id).await?;
        } else {
            info!("npalist: current_min_id is None, skipping manifest update");
        }

        // Если нашли новые элементы на offset=0, возвращаем их
        if !latest_not_published.is_empty() {
            info!(
                count = latest_not_published.len(),
                "npalist: latest page has new items, no need for deep dive"
            );
            return Ok(());
        }

        // 2. Если новых элементов нет, углубляемся в историю
        // Вычисляем точный offset для пропуска уже опубликованных страниц
        info!(current_max_id = current_max_id, min_published_id = min_published_project_id, "npalist: calculating history offset for streaming");
        let history_offset = if let Some(min_id) = min_published_project_id {
            if let Some(current_max) = current_max_id {
                // Проверяем, что min_id не больше current_max
                if min_id > current_max {
                    info!(
                        current_max_id = current_max,
                        min_published_id = min_id,
                        "npalist: min_published_project_id is greater than current_max_id, starting from limit"
                    );
                    limit
                } else {
                    // Вычисляем offset: current_max - min_id
                    // Это дает точный offset первого неопубликованного элемента
                    let offset = current_max - min_id;
                    info!(
                        current_max_id = current_max,
                        min_published_id = min_id,
                        calculated_offset = offset,
                        "npalist: calculated history offset to skip published pages"
                    );
                    offset
                }
            } else {
                // Если не можем вычислить, начинаем с limit
                limit
            }
        } else {
            // Если нет min_published_project_id, начинаем с limit
            limit
        };

        // 3. Углубляемся в историю
        let mut current_offset = history_offset;
        let mut processed_history_items: Vec<CrawlItem> = Vec::new();
        
        loop {
            let url_cont = self
                .url_template
                .replace("{limit}", &limit.to_string())
                .replace("{offset}", &current_offset.to_string());
            info!(%url_cont, current_offset, "npalist: deep dive into history for streaming");

            let history_page = self.client.get(&url_cont).send().await?;
            info!(status = %history_page.status(), "npalist: history page response status");
            
            if !history_page.status().is_success() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("npalist: http error on history: {}", history_page.status()),
                )));
            }
            
            let history_page_text = history_page.text().await?;
            info!(text_len = history_page_text.len(), "npalist: history page response text length");
            let history_projects = parse_npa_projects(&history_page_text, self.project_id_re.as_ref());

            // Если страница пустая, значит дошли до конца истории
            if history_projects.is_empty() {
                info!("npalist: reached end of history, no more pages");
                break;
            }

            info!(count = history_projects.len(), "npalist: parsing history projects for streaming");
            
            // Отправляем элементы по одному, если они не полностью опубликованы
            let mut found_new_items = false;
            for it in history_projects.into_iter() {
                if let Some(pid) = it.project_id.as_deref() {
                    if let Ok(pid_num) = pid.parse::<u32>() {
                        // Проверяем, полностью ли опубликован элемент
                        let fully_published = self.cache_manager.is_fully_published(pid, &self.enabled_channels).await?;
                        if fully_published {
                            info!(project_id = pid_num, "npalist: history project is fully published, skipping");
                        } else {
                            info!(project_id = pid_num, "npalist: history project not fully published, sending to worker");
                            found_new_items = true;
                            processed_history_items.push(it.clone());
                            // Отправляем элемент в канал (может зависнуть если канал полон)
                            if let Err(_) = sender.send(it).await {
                                info!("npalist: worker channel closed, stopping streaming");
                                return Ok(());
                            }
                        }
                    }
                }
            }
            
            // Если новых элементов нет, продолжаем углубление
            if !found_new_items {
                current_offset += limit;
                if self.poll_delay.as_millis() > 0 {
                    info!(
                        delay_ms = self.poll_delay.as_millis(),
                        current_offset,
                        "npalist: sleeping before next history page request to avoid rate limiting"
                    );
                    tokio::time::sleep(self.poll_delay).await;
                }
            } else {
                // Нашли новые элементы, можно остановиться
                break;
            }
        }
        
        // Обновляем min_published_project_id в manifest после обработки истории
        let history_min_id = processed_history_items.iter()
            .filter_map(|item| item.project_id.as_deref())
            .filter_map(|pid| pid.parse::<u32>().ok())
            .min();
            
        if let Some(new_min_id) = [current_min_id, history_min_id]
            .iter()
            .filter_map(|&id| id)
            .min() {
            let mut updated_manifest = self.cache_manager.load_manifest().await?;
            updated_manifest.min_published_project_id = Some(new_min_id);
            info!(new_min_id = new_min_id, "npalist: updated min_published_project_id after history processing");
            self.cache_manager.save_manifest(&updated_manifest).await?;
        }
        
        Ok(())
    }
}


fn parse_npa_projects(text: &str, project_id_re: Option<&Regex>) -> Vec<CrawlItem> {
    let mut out = Vec::new();
    info!(text_len = text.len(), "parse_npa_projects: input text length");
    let preview: String = text.chars().take(200).collect();
    info!(text_preview = %preview, "parse_npa_projects: input text preview");
    let doc = match Document::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            error!(error = %e, "parse_npa_projects: XML parsing failed");
            return Vec::new();
        }
    };
    let project_nodes: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("project")).collect();
    info!(project_count = project_nodes.len(), "parse_npa_projects: found project nodes");
    for proj in project_nodes {
        let mut project_attr_id = proj.attribute("id").unwrap_or("").to_string();
        if project_attr_id.is_empty() {
            info!("parse_npa_projects: skipping project with empty id");
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
            (None, None) => {
                continue;
            },
        };
        let mut url = format!("https://regulation.gov.ru/projects/{}", project_attr_id);
        if let Some(re) = project_id_re {
            // Проверяем соответствие по regex: пытаемся извлечь id из полного URL
            let full_url = format!("https://regulation.gov.ru/projects/{}", project_attr_id);
            if let Some(cap) = re.captures(&full_url).and_then(|c| c.get(1)) {
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
