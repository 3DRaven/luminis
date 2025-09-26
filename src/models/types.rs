use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use derive_more::{From, Into, Display, AsRef, FromStr};
use bon::bon;
use strum_macros::Display as StrumDisplay;

/// Идентификатор проекта
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, From, Into, Display, AsRef, FromStr)]
#[from(String, &str)]
pub struct ProjectId(String);

#[bon]
impl ProjectId {
    #[builder]
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Путь к файлу DOCX
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, From, Into, AsRef, FromStr)]
#[from(PathBuf, String, &str)]
pub struct DocxPath(PathBuf);

#[bon]
impl DocxPath {
    #[builder]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }

    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

/// Путь к файлу Markdown
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, From, Into, AsRef)]
#[from(PathBuf, String, &str)]
pub struct MarkdownPath(PathBuf);

#[bon]
impl MarkdownPath {
    #[builder]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }

    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

/// Путь к файлу с суммаризацией
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, From, Into, AsRef)]
#[from(PathBuf, String, &str)]
pub struct SummaryPath(PathBuf);

#[bon]
impl SummaryPath {
    #[builder]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }

    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

/// Путь к файлу с постом
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, From, Into, AsRef)]
#[from(PathBuf, String, &str)]
pub struct PostPath(PathBuf);

#[bon]
impl PostPath {
    #[builder]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &PathBuf {
        &self.0
    }

    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

/// Текст суммаризации
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From, Into, Display, AsRef, FromStr)]
#[from(String, &str)]
pub struct SummaryText(String);

#[bon]
impl SummaryText {
    #[builder]
    pub fn new(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Текст поста
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From, Into, Display, AsRef)]
#[from(String, &str)]
pub struct PostText(String);

#[bon]
impl PostText {
    #[builder]
    pub fn new(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Текст Markdown
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From, Into, Display, AsRef)]
#[from(String, &str)]
pub struct MarkdownText(String);

#[bon]
impl MarkdownText {
    #[builder]
    pub fn new(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Временная метка создания
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, From, Into, Display, AsRef)]
#[from(String, &str)]
pub struct CreatedAt(String);

#[bon]
impl CreatedAt {
    #[builder]
    pub fn new(timestamp: String) -> Self {
        Self(timestamp)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Manifest {
    #[serde(default)]
    pub min_published_project_id: Option<u32>,
}

impl Manifest {
    pub fn new() -> Self {
        Self::default()
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

#[derive(Clone, Debug, StrumDisplay, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct CacheMetadata {
    pub project_id: ProjectId,
    pub docx_path: DocxPath,
    pub markdown_path: MarkdownPath,
    pub published_channels: Vec<crate::models::channel::PublisherChannel>,
    pub created_at: CreatedAt,
    // Новые поля для суммаризаций по каналам
    pub channel_summaries: std::collections::HashMap<crate::models::channel::PublisherChannel, SummaryText>, // channel -> summary_text
    pub channel_posts: std::collections::HashMap<crate::models::channel::PublisherChannel, PostText>,     // channel -> post_text
    // Метаданные из NpaListCrawler
    pub crawl_metadata: Vec<MetadataItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id() {
        let id = ProjectId::from("test-project");
        assert_eq!(id.as_str(), "test-project");
        assert_eq!(id.to_string(), "test-project");
        
        // Test FromStr
        let id_from_str: ProjectId = "test-project".parse().unwrap();
        assert_eq!(id_from_str, id);
    }

    #[test]
    fn test_path_types() {
        let docx_path = DocxPath::from("/path/to/file.docx");
        assert_eq!(docx_path.as_path(), &PathBuf::from("/path/to/file.docx"));

        let markdown_path = MarkdownPath::from("/path/to/file.md");
        assert_eq!(markdown_path.as_path(), &PathBuf::from("/path/to/file.md"));
        
        // Test FromStr for paths
        let docx_from_str: DocxPath = "/path/to/file.docx".parse().unwrap();
        assert_eq!(docx_from_str, docx_path);
    }

    #[test]
    fn test_text_types() {
        let summary = SummaryText::from("Test summary");
        assert_eq!(summary.as_str(), "Test summary");
        assert_eq!(summary.to_string(), "Test summary");
        assert!(!summary.is_empty());

        let empty_summary = SummaryText::from("");
        assert!(empty_summary.is_empty());
        
        // Test FromStr
        let summary_from_str: SummaryText = "Test summary".parse().unwrap();
        assert_eq!(summary_from_str, summary);
    }
}
