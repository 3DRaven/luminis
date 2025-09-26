use std::fs;
use std::path::Path;
use crate::models::config::AppConfig;

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<AppConfig, Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path)?;
    let cfg: AppConfig = serde_yaml::from_str(&content)?;
    Ok(cfg)
}


