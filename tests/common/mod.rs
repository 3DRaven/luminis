use std::fs;
use std::path::PathBuf;
use tera::{Tera, Context};
use tempfile;
use wiremock::{Mock, ResponseTemplate, MockServer};
use wiremock::matchers::{method, path, path_regex, query_param};

/// Загружает шаблон конфигурации для тестов
fn load_test_config_template() -> String {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/resources/configs/mastodon_telegram.yaml");
    fs::read_to_string(config_path).unwrap()
}

/// Загружает моки для тестов
pub fn read_mocks() -> (String, String) {
    let rss_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/rss.xml");
    let stages_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/stages.json");
    
    let rss_xml = fs::read_to_string(rss_path).unwrap();
    let stages_json = fs::read_to_string(stages_path).unwrap();
    
    (rss_xml, stages_json)
}

pub async fn mount_rss(server: &MockServer, rss_xml: &str) {
    let mock = Mock::given(method("GET"))
        .and(path("/api/public/Rss"))
        .respond_with(ResponseTemplate::new(200).set_body_string(rss_xml));
    server.register(mock).await;
}

pub async fn mount_rss_with_error(server: &MockServer) {
    let mock = Mock::given(method("GET"))
        .and(path("/api/public/Rss"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"));
    server.register(mock).await;
}

pub async fn mount_npalist(server: &MockServer) {
    let npalist_xml = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/npalist.xml"),
    )
    .unwrap();
    let mock = Mock::given(method("GET"))
        .and(path_regex(r"/api/npalist/"))
        .and(query_param("limit", "50"))
        .and(query_param("offset", "0"))
        .and(query_param("sort", "desc"))
        .respond_with(ResponseTemplate::new(200).set_body_string(npalist_xml));
    server.register(mock).await;
}

pub async fn mount_npalist_with_error(server: &MockServer) {
    let mock = Mock::given(method("GET"))
        .and(path_regex(r"/api/npalist/"))
        .and(query_param("limit", "50"))
        .and(query_param("offset", "0"))
        .and(query_param("sort", "desc"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"));
    server.register(mock).await;
}

pub async fn mount_stages(server: &MockServer, stages_json: &str) {
    let mock = Mock::given(method("GET"))
        .and(path_regex(r"/api/public/PublicProjects/GetProjectStages/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_string(stages_json));
    server.register(mock).await;
}

pub async fn mount_docx(server: &MockServer) {
    let docx_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/source.docx");
    let docx_content = fs::read(&docx_path).unwrap();
    
    let mock = Mock::given(method("GET"))
        .and(path_regex(r"/api/public/Files/GetFile"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
                .set_body_bytes(docx_content)
        );
    server.register(mock).await;
}

pub async fn mount_gemini_generate(server: &MockServer) {
    let response_body = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "tests/resources/mocks/body-v1beta-models-gemini-2.0-flash_generateContent-8OOhY.json",
        ),
    )
    .unwrap();
    let mock = Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-2.0-flash:generateContent"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json; charset=UTF-8")
                .set_body_string(response_body)
        );
    server.register(mock).await;
}

pub async fn mount_mastodon(server: &MockServer) {
    let mstd_json = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/mastodon_status.json"),
    )
    .unwrap();
    let mock = Mock::given(method("POST"))
        .and(path("/api/v1/statuses"))
        .respond_with(ResponseTemplate::new(200).set_body_string(mstd_json));
    server.register(mock).await;
}

/// Создает мок Mastodon с проверкой конкретных параметров запроса
pub async fn mount_mastodon_with_params_check(
    server: &MockServer,
    expected_visibility: Option<&str>,
    expected_language: Option<&str>,
    expected_sensitive: Option<bool>,
) {
    let mstd_json = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/mastodon_status.json"),
    )
    .unwrap();
    
    let mut mock_builder = Mock::given(method("POST"))
        .and(path("/api/v1/statuses"));
    
    // Добавляем проверки параметров если они указаны
    if let Some(visibility) = expected_visibility {
        mock_builder = mock_builder.and(query_param("visibility", visibility));
    }
    if let Some(language) = expected_language {
        mock_builder = mock_builder.and(query_param("language", language));
    }
    if let Some(sensitive) = expected_sensitive {
        mock_builder = mock_builder.and(query_param("sensitive", if sensitive { "true" } else { "false" }));
    }
    
    let mock = mock_builder.respond_with(ResponseTemplate::new(200).set_body_string(mstd_json));
    server.register(mock).await;
}

pub async fn mount_telegram(server: &MockServer) {
    let mock = Mock::given(method("POST"))
        .and(path_regex(r"/botTEST/sendMessage"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"ok\":true}"));
    server.register(mock).await;
}

pub fn render_config(
    base: &str,
    out_path: &str,
    cache_dir: &str,
    mastodon_enabled: bool,
    telegram_enabled: bool,
    console_enabled: bool,
    file_enabled: bool,
    rss_enabled: bool,
    npalist_enabled: bool,
) -> tempfile::NamedTempFile {
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", &out_path);
    ctx.insert("cache", &cache_dir);
    ctx.insert("mastodon_enabled", &mastodon_enabled);
    ctx.insert("telegram_enabled", &telegram_enabled);
    ctx.insert("console_enabled", &console_enabled);
    ctx.insert("file_enabled", &file_enabled);
    ctx.insert("rss_enabled", &rss_enabled);
    ctx.insert("npalist_enabled", &npalist_enabled);
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    cfg_file
}

pub fn render_config_with_retry_limit(
    base: &str,
    out_path: &str,
    cache_dir: &str,
    mastodon_enabled: bool,
    telegram_enabled: bool,
    console_enabled: bool,
    file_enabled: bool,
    rss_enabled: bool,
    npalist_enabled: bool,
    max_retry_attempts: u64,
) -> tempfile::NamedTempFile {
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", &out_path);
    ctx.insert("cache", &cache_dir);
    ctx.insert("mastodon_enabled", &mastodon_enabled);
    ctx.insert("telegram_enabled", &telegram_enabled);
    ctx.insert("console_enabled", &console_enabled);
    ctx.insert("file_enabled", &file_enabled);
    ctx.insert("rss_enabled", &rss_enabled);
    ctx.insert("npalist_enabled", &npalist_enabled);
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    ctx.insert("max_retry_attempts", &max_retry_attempts);
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    cfg_file
}

pub fn prepopulate_cache(cache_dir: &str, project_id: &str, summary_text: &str) {
    let cache_path = PathBuf::from(cache_dir).join(format!("{}.json", project_id));
    let cache_data = serde_json::json!({
        "summary": summary_text,
        "timestamp": chrono::Utc::now().timestamp()
    });
    fs::write(cache_path, serde_json::to_string_pretty(&cache_data).unwrap()).unwrap();
}

pub async fn mount_gemini_generate_with_limit(server: &MockServer, _limit: usize) {
    let response_body = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "tests/resources/mocks/body-v1beta-models-gemini-2.0-flash_generateContent-8OOhY.json",
        ),
    )
    .unwrap();
    let mock = Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-2.0-flash:generateContent"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json; charset=UTF-8")
                .set_body_string(response_body)
        );
    server.register(mock).await;
}

pub fn prepopulate_channel_cache(
    cache_dir: &str,
    project_id: &str,
    channel: &str,
    summary_text: &str,
) {
    let cache_path = PathBuf::from(cache_dir)
        .join(format!("{}_{}.json", project_id, channel));
    let cache_data = serde_json::json!({
        "summary": summary_text,
        "timestamp": chrono::Utc::now().timestamp()
    });
    fs::write(cache_path, serde_json::to_string_pretty(&cache_data).unwrap()).unwrap();
}

pub fn render_config_with_channels(
    base: &str,
    out_path: &str,
    cache_dir: &str,
    mastodon_enabled: bool,
    telegram_enabled: bool,
    console_enabled: bool,
    file_enabled: bool,
) -> tempfile::NamedTempFile {
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", &out_path);
    ctx.insert("cache", &cache_dir);
    ctx.insert("mastodon_enabled", &mastodon_enabled);
    ctx.insert("telegram_enabled", &telegram_enabled);
    ctx.insert("console_enabled", &console_enabled);
    ctx.insert("file_enabled", &file_enabled);
    ctx.insert("rss_enabled", &false);
    ctx.insert("npalist_enabled", &true);
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    cfg_file
}

/// Рендерит конфигурацию с кастомными лимитами символов
pub fn render_config_with_custom_limits(
    base: &str,
    out_path: &str,
    cache_dir: &str,
    mastodon_enabled: bool,
    telegram_enabled: bool,
    console_enabled: bool,
    file_enabled: bool,
    telegram_max_chars: usize,
    mastodon_max_chars: usize,
    console_max_chars: usize,
    file_max_chars: usize,
) -> tempfile::NamedTempFile {
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", &out_path);
    ctx.insert("cache", &cache_dir);
    ctx.insert("mastodon_enabled", &mastodon_enabled);
    ctx.insert("telegram_enabled", &telegram_enabled);
    ctx.insert("console_enabled", &console_enabled);
    ctx.insert("file_enabled", &file_enabled);
    ctx.insert("rss_enabled", &true);
    ctx.insert("npalist_enabled", &true);
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    ctx.insert("telegram_max_chars", &telegram_max_chars);
    ctx.insert("mastodon_max_chars", &mastodon_max_chars);
    ctx.insert("console_max_chars", &console_max_chars);
    ctx.insert("file_max_chars", &file_max_chars);
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    cfg_file
}

/// Создает конфигурацию с кастомными параметрами Mastodon
pub fn render_config_with_mastodon_params(
    base: &str,
    out_path: &str,
    cache_dir: &str,
    mastodon_enabled: bool,
    telegram_enabled: bool,
    console_enabled: bool,
    file_enabled: bool,
    mastodon_visibility: Option<&str>,
    mastodon_language: Option<&str>,
    mastodon_sensitive: Option<bool>,
    mastodon_max_chars: Option<usize>,
) -> tempfile::NamedTempFile {
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", &out_path);
    ctx.insert("cache", &cache_dir);
    ctx.insert("mastodon_enabled", &mastodon_enabled);
    ctx.insert("telegram_enabled", &telegram_enabled);
    ctx.insert("console_enabled", &console_enabled);
    ctx.insert("file_enabled", &file_enabled);
    ctx.insert("rss_enabled", &true);
    ctx.insert("npalist_enabled", &true);
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    
    // Добавляем кастомные параметры Mastodon
    if let Some(visibility) = mastodon_visibility {
        ctx.insert("mastodon_visibility", &visibility);
    }
    if let Some(language) = mastodon_language {
        ctx.insert("mastodon_language", &language);
    }
    if let Some(sensitive) = mastodon_sensitive {
        ctx.insert("mastodon_sensitive", &sensitive);
    }
    if let Some(max_chars) = mastodon_max_chars {
        ctx.insert("mastodon_max_chars", &max_chars);
    }
    
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    cfg_file
}
