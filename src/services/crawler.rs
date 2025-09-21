use std::time::Duration;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use regex::Regex;
use reqwest::Client;
use tracing::info;
use serde::{Serialize, Deserialize};
use roxmltree::Document;

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
    pub fn path() -> &'static str { "./cache/manifest.json" }

    pub fn load() -> Manifest {
        let p = Path::new(Self::path());
        if p.exists() {
            if let Ok(s) = fs::read_to_string(p) {
                if let Ok(m) = serde_json::from_str::<Manifest>(&s) { return m; }
            }
        }
        Manifest::default()
    }

    pub fn save(&self) {
        // Ensure cache dir exists
        if let Some(dir) = Path::new(Self::path()).parent() { let _ = fs::create_dir_all(dir); }
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        let _ = fs::write(Self::path(), json);
    }
}

/// Scanner for RSS feed: parses XML and extracts CrawlItem with metadata from description
pub struct RssScanner {
    client: Client,
}

impl RssScanner {
    pub fn new(timeout: Duration) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self { client })
    }

    pub async fn fetch(&self, url: &str, re: &Regex) -> Result<Vec<CrawlItem>, Box<dyn std::error::Error + Send + Sync>> {
        info!(%url, "rss: fetch");
        let xml = self.client.get(url).send().await?.text().await?;
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
            // Try to extract project_id in order: guid -> link -> description, using provided regex.
            let mut pid: Option<String> = None;
            if let Some(g) = guid.as_deref() {
                pid = re.captures(g).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
            }
            if pid.is_none() {
                if let Some(l) = link.as_deref() {
                    pid = re.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
                    if pid.is_none() {
                        pid = default_link_re.captures(l).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
                    }
                }
            }
            if pid.is_none() {
                if let Some(d) = desc.as_deref() {
                    pid = re.captures(d).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
                }
            }
            if let Some(project_id) = pid {
                let (status, stage, kind, department, responsible, publish_date) = Self::parse_description(desc.as_deref());
                let url = link.clone().unwrap_or_default();
                let title_val = title.clone().unwrap_or_default();
                if url.is_empty() || title_val.is_empty() { continue; }
                items.push(CrawlItem {
                    title: title_val,
                    url,
                    body: String::new(),
                    project_id: Some(project_id),
                    date: None,
                    publish_date,
                    regulatory_impact: None,
                    regulatory_impact_id: None,
                    responsible,
                    department,
                    department_id: None,
                    status,
                    status_id: None,
                    stage,
                    stage_id: None,
                    kind,
                    kind_id: None,
                    procedure: None,
                    procedure_id: None,
                    procedure_result: None,
                    procedure_result_id: None,
                    next_stage_duration: None,
                    parallel_stage_start_discussion: None,
                    parallel_stage_end_discussion: None,
                    start_discussion: None,
                    end_discussion: None,
                    problem: None,
                    objectives: None,
                    circle_persons: None,
                    social_relations: None,
                    rationale: None,
                    transition_period: None,
                    plan_date: None,
                    complite_date_act: None,
                    complite_number_dep_act: None,
                    complite_number_reg_act: None,
                    parallel_stage_files: Vec::new(),
                });
            }
        }
        Ok(items)
    }

    fn parse_description(desc: Option<&str>) -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) {
        let status = None;
        let stage = None;
        let mut kind = None;
        let department = None;
        let mut responsible = None;
        let mut publish_date = None;
        if let Some(d) = desc {
            for line in d.lines() {
                let line = line.trim();
                if line.starts_with("Вид:") { kind = Some(line.trim_start_matches("Вид:").trim().trim_matches('"').to_string()); }
                if line.starts_with("Процедура:") { /* could map to procedure if needed */ }
                if line.starts_with("Разработчик:") { responsible = Some(line.trim_start_matches("Разработчик:").trim().trim_matches('"').to_string()); }
                if line.starts_with("Дата создания:") { publish_date = Some(line.trim_start_matches("Дата создания:").trim().to_string()); }
            }
        }
        (status, stage, kind, department, responsible, publish_date)
    }
}

/// Scanner for NPA list API with pagination stored in manifest.json
pub struct NpaListScanner {
    client: Client,
}

impl NpaListScanner {
    pub fn new(timeout: Duration) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self { client })
    }

    pub async fn fetch(&self, url_template: &str, limit_opt: Option<u32>, project_id_re: Option<&Regex>) -> Result<Vec<CrawlItem>, Box<dyn std::error::Error + Send + Sync>> {
        let mut manifest = Manifest::load();
        let key = url_template.to_string();
        let limit = limit_opt.unwrap_or(50);
        let state = manifest.sources.get(&key).cloned().unwrap_or_default();
        let last_offset = state.last_offset;

        // First fetch latest page (offset=0)
        let url_latest = url_template
            .replace("{limit}", &limit.to_string())
            .replace("{offset}", &0.to_string());
        info!(%url_latest, "npalist: fetch latest page (offset=0)");
        let latest_text = self.client.get(&url_latest).send().await?.text().await?;
        let mut latest = Self::parse_projects(&latest_text, project_id_re);

        // Then fetch from last_offset to continue deep paging
        let url_cont = url_template
            .replace("{limit}", &limit.to_string())
            .replace("{offset}", &last_offset.to_string());
        info!(%url_cont, last_offset, "npalist: fetch continue page");
        let cont_text = self.client.get(&url_cont).send().await?.text().await?;
        let mut cont = Self::parse_projects(&cont_text, project_id_re);

        // Merge: latest first, then continuation, avoiding duplicates by project_id (simple stable merge)
        latest.append(&mut cont);

        // Update offset only if continuation had data
        if last_offset == 0 || !latest.is_empty() {
            let entry = manifest.sources.entry(key).or_default();
            entry.last_limit = limit;
            entry.last_offset = last_offset.saturating_add(limit);
            manifest.save();
        }
        Ok(latest)
    }

    fn parse_projects(text: &str, project_id_re: Option<&Regex>) -> Vec<CrawlItem> {
        let mut out = Vec::new();
        let doc = Document::parse(text).unwrap_or_else(|_| Document::parse("<projects/>").unwrap());
        for proj in doc.descendants().filter(|n| n.has_tag_name("project")) {
            let mut project_attr_id = proj.attribute("id").unwrap_or("").to_string();
            if project_attr_id.is_empty() { continue; }
            let text_of = |name: &str| -> Option<String> {
                proj.children().find(|n| n.has_tag_name(name)).and_then(|n| n.text()).map(|s| s.trim().to_string())
            };
            let text_and_id = |name: &str| -> (Option<String>, Option<String>) {
                if let Some(node) = proj.children().find(|n| n.has_tag_name(name)) {
                    (node.text().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()), node.attribute("id").map(|v| v.to_string()))
                } else { (None, None) }
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
                // Проверяем соответствие по regex: пытаемся извлечь id из URL
                if let Some(cap) = re.captures(&url).and_then(|c| c.get(1)) {
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
            let parallel_files: Vec<String> = proj.children().filter(|n| n.has_tag_name("parallelStageFile")).filter_map(|n| n.text().map(|s| s.trim().to_string())).collect();

            let mut body_lines: Vec<String> = Vec::new();
            if let Some(d) = text_of("date") { body_lines.push(format!("Дата: {}", d)); }
            if let Some(pd) = text_of("publishDate") { body_lines.push(format!("Публикация: {}", pd)); }
            if let Some(s) = &stage_text { body_lines.push(format!("Стадия: {}{}", s, stage_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &status_text { body_lines.push(format!("Статус: {}{}", s, status_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &ri_text { body_lines.push(format!("Рег. влияние: {}{}", s, ri_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &pr_text { body_lines.push(format!("Результат процедуры: {}{}", s, pr_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &kind_text { body_lines.push(format!("Вид: {}{}", s, kind_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &dept_text { body_lines.push(format!("Ведомство: {}{}", s, dept_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }
            if let Some(s) = &proc_text { body_lines.push(format!("Процедура: {}{}", s, proc_id.as_ref().map(|v| format!(" (id: {})", v)).unwrap_or_default())); }

            let body = if body_lines.is_empty() { String::new() } else { format!("{}\n{}", title, body_lines.join("\n")) };
            out.push(CrawlItem {
                title,
                url,
                body,
                project_id: Some(project_attr_id),
                date: text_of("date"),
                publish_date: text_of("publishDate"),
                regulatory_impact: ri_text,
                regulatory_impact_id: ri_id,
                responsible: text_of("responsible"),
                department: dept_text,
                department_id: dept_id,
                status: status_text,
                status_id: status_id,
                stage: stage_text,
                stage_id: stage_id,
                kind: kind_text,
                kind_id: kind_id,
                procedure: proc_text,
                procedure_id: proc_id,
                procedure_result: pr_text,
                procedure_result_id: pr_id,
                next_stage_duration: text_of("nextStageDuration"),
                parallel_stage_start_discussion: text_of("parallelStageStartDiscussion"),
                parallel_stage_end_discussion: text_of("parallelStageEndDiscussion"),
                start_discussion: text_of("startDiscussion"),
                end_discussion: text_of("endDiscussion"),
                problem: text_of("problem"),
                objectives: text_of("objectives"),
                circle_persons: text_of("circlePersons"),
                social_relations: text_of("socialRelations"),
                rationale: text_of("rationale"),
                transition_period: text_of("transitionPeriod"),
                plan_date: text_of("planDate"),
                complite_date_act: text_of("compliteDateAct"),
                complite_number_dep_act: text_of("compliteNumberDepAct"),
                complite_number_reg_act: text_of("compliteNumberRegAct"),
                parallel_stage_files: parallel_files,
            });
        }
        out
    }
}

/// Scanner for stages endpoint: extracts fileId and may enrich metadata later
pub struct FileIdScanner {
    client: Client,
}

impl FileIdScanner {
    pub fn new() -> Self { Self { client: Client::new() } }

    pub async fn fetch_file_id(&self, url: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        info!(%url, "fileid: fetch");
        let body = self.client.get(url).send().await?.text().await?;
        let re = Regex::new(r#"fileId"\s*:\s*"([^"]+)"#).unwrap();
        for caps in re.captures_iter(&body) {
            if let Some(m) = caps.get(1) { return Ok(Some(m.as_str().to_string())); }
        }
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub struct CrawlItem {
    pub title: String,
    pub url: String,
    pub body: String,
    pub project_id: Option<String>,
    pub date: Option<String>,
    pub publish_date: Option<String>,
    pub regulatory_impact: Option<String>,
    pub regulatory_impact_id: Option<String>,
    pub responsible: Option<String>,
    pub department: Option<String>,
    pub department_id: Option<String>,
    pub status: Option<String>,
    pub status_id: Option<String>,
    pub stage: Option<String>,
    pub stage_id: Option<String>,
    pub kind: Option<String>,
    pub kind_id: Option<String>,
    pub procedure: Option<String>,
    pub procedure_id: Option<String>,
    pub procedure_result: Option<String>,
    pub procedure_result_id: Option<String>,
    pub next_stage_duration: Option<String>,
    pub parallel_stage_start_discussion: Option<String>,
    pub parallel_stage_end_discussion: Option<String>,
    pub start_discussion: Option<String>,
    pub end_discussion: Option<String>,
    pub problem: Option<String>,
    pub objectives: Option<String>,
    pub circle_persons: Option<String>,
    pub social_relations: Option<String>,
    pub rationale: Option<String>,
    pub transition_period: Option<String>,
    pub plan_date: Option<String>,
    pub complite_date_act: Option<String>,
    pub complite_number_dep_act: Option<String>,
    pub complite_number_reg_act: Option<String>,
    pub parallel_stage_files: Vec<String>,
}

// Old crawler trait and structs removed in favor of specialized scanners


