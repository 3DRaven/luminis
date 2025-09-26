use async_trait::async_trait;
use std::error::Error;

use super::utils::trim_with_ellipsis;
use crate::traits::publisher::Publisher;

pub struct FilePublisher {
    pub path: String,
    pub max_chars: Option<usize>,
    pub append: bool,
}

#[async_trait]
impl Publisher for FilePublisher {
    fn name(&self) -> &str { "file" }
    async fn publish(&self, _title: &str,_urll: &str, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let final_text = if let Some(maxc) = self.max_chars { trim_with_ellipsis(text, maxc) } else { text.to_string() };
        let p = std::path::Path::new(&self.path);
        if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
        if self.append {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
            writeln!(f, "{}", final_text)?;
        } else {
            std::fs::write(p, format!("{}\n", final_text))?;
        }
        Ok(())
    }
}
