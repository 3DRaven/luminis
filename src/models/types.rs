use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use derive_more::{From, Into, Display, AsRef, FromStr};
use bon::bon;

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
