use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub telegram: Option<TelegramConfig>,
    pub llm: LlmConfig,
    pub crawler: CrawlerConfig,
    pub mastodon: Option<MastodonConfig>,
    pub output: Option<OutputConfig>,
    pub run: Option<RunConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramConfig {
    pub api_base_url: String,
    pub bot_token: String,
    pub target_chat_id: i64,
    pub enabled: bool,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub model: Option<String>,
    pub use_local: Option<bool>,     // if true, use local kalosm
    pub model_path: Option<String>,  // absolute or relative path to .gguf
    pub tokenizer_path: Option<String>, // optional path to tokenizer.json
    pub variant: Option<String>,     // "base" | "q80" (LMRS variants)
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_new_tokens: Option<usize>,
    pub seed: Option<u64>,
    // Prompt/attention optimizations
    pub sliding_window: Option<usize>,            // ограничить размер окна attention
    pub prompt_compression_ratio: Option<f32>,    // 0.0..=1.0, сжатие длины промпта по токенам
    pub enable_prompt_cache: Option<bool>,        // включить кэш префикса
    // Similarity/LSH (gaoya MinHash) options
    pub enable_similarity_index: Option<bool>,    // включить MinHash-индекс эмбеддингов
    pub minhash_num_bands: Option<usize>,
    pub minhash_band_width: Option<usize>,
    pub minhash_jaccard_threshold: Option<f32>,   // 0.0..=1.0
    // ai-lib cloud/provider options
    pub provider: Option<String>,                 // "OpenAI" | "Groq" | ...
    pub base_url: Option<String>,
    pub proxy: Option<String>,
    pub api_key: Option<String>,
    pub request_timeout_secs: Option<u64>,
    // Logging options
    pub log_prompt_preview_chars: Option<usize>,  // сколько символов промпта логировать
}

#[derive(Debug, Deserialize, Clone)]
pub struct CrawlerConfig {
    pub interval_seconds: u64,
    pub request_timeout_secs: Option<u64>,
    pub poll_delay_secs: Option<u64>,
    pub npalist: Option<NpaListConfig>,
    pub rss: Option<RssConfig>,
    pub file_id: Option<FileIdConfig>,
}

// RSS sources
#[derive(Debug, Deserialize, Clone)]
pub struct RssConfig {
    pub enabled: Option<bool>,
    pub url: String,
    pub regex: String,
}

// NPA list sources (API)
#[derive(Debug, Deserialize, Clone)]
pub struct NpaListConfig {
    pub enabled: Option<bool>,
    pub url: String,
    pub limit: Option<u32>,
    pub regex: Option<String>,
}


#[derive(Debug, Deserialize, Clone)]
pub struct FileIdConfig {
    pub url: String,   // e.g. https://.../GetProjectStages/{project_id}
    pub regex: String,          // regex with capture group for fileId
}

#[derive(Debug, Deserialize, Clone)]
pub struct MastodonConfig {
    pub base_url: String,        // https://mastodon.social
    pub access_token: String,    // user/app token
    pub enabled: bool,
    pub login_cli: Option<bool>, // prompt for token on startup if empty
    pub visibility: Option<String>, // public | unlisted | private | direct
    pub language: Option<String>,   // e.g. ru, en
    pub spoiler_text: Option<String>, // default "Новости"
    pub sensitive: Option<bool>,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub console_enabled: Option<bool>,
    pub file_enabled: Option<bool>,
    pub file_path: Option<String>,
    pub console_max_chars: Option<usize>,
    pub file_max_chars: Option<usize>,
    pub file_append: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RunConfig {
    pub single_shot: Option<bool>,
    pub max_posts_per_run: Option<usize>,
    pub summarization_timeout_secs: Option<u64>,
    pub input_sample_percent: Option<f32>, // 0.0..=1.0, how much of docx text to feed LLM
    pub model_max_chars: Option<usize>,    // soft limit for summarizer prompt
    pub hard_max_chars: Option<usize>,     // deprecated; not used
    pub prompt_template: Option<String>,   // Tera template for summarizer prompt
    pub cache_dir: Option<String>,         // directory for caching artifacts
    pub post_template: Option<String>,     // Tera template for final post formatting
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<AppConfig, Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path)?;
    let cfg: AppConfig = serde_yaml::from_str(&content)?;
    Ok(cfg)
}


