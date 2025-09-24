use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, path_regex, query_param};
use luminis::services::documents::save_cache_artifacts;

pub fn read_mocks() -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rss = fs::read_to_string(root.join("tests/resources/mocks/rss.xml")).unwrap();
    let stages = fs::read_to_string(root.join("tests/resources/mocks/stages.json")).unwrap();
    (rss, stages)
}

pub fn load_test_config_template() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/configs/mastodon_telegram.yaml");
    fs::read_to_string(p).unwrap()
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
        .and(path_regex(r"/api/npalist/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(npalist_xml));
    server.register(mock).await;
}

pub async fn mount_npalist_with_error(server: &MockServer) {
    let mock = Mock::given(method("GET"))
        .and(path("/api/npalist/"))
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
    // Сохраняем и markdown данные, и суммаризацию для полного кэша
    let markdown_text = "Тестовый markdown текст для проекта";
    save_cache_artifacts(
        cache_dir,
        project_id,
        None,
        markdown_text,
        summary_text,
        "",
        &[],
    )
    .unwrap();
}

/// Создает мок для Gemini с указанным лимитом символов
pub async fn mount_gemini_generate_with_limit(server: &MockServer, limit: usize) {
    let response_body = format!(
        r#"{{"candidates":[{{"content":{{"parts":[{{"text":"Краткая суммаризация для лимита {} символов. Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.\\n\\nРейтинг:\\nПолезность: 5/10 (частично улучшает ОМС)\\nРепрессивность: 2/10 (незначительно)\\nКоррупц. емкость: 6/10 (регион. перераспределение)"}}]}}}}]}}"#,
        limit
    );
    
    let mock = Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-2.0-flash:generateContent"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(response_body)
        );
    server.register(mock).await;
}

/// Предзаполняет кэш для конкретного канала
pub fn prepopulate_channel_cache(
    cache_dir: &str,
    project_id: &str,
    channel: &str,
    summary_text: &str,
) {
    use std::collections::HashMap;
    use serde_json;
    use std::path::PathBuf;
    
    // Создаем директорию проекта
    let project_dir = PathBuf::from(cache_dir).join(project_id);
    std::fs::create_dir_all(&project_dir).unwrap();
    
    // Создаем metadata.json с суммаризацией для канала
    let mut channel_summaries = HashMap::new();
    channel_summaries.insert(channel.to_string(), summary_text.to_string());
    
    let metadata = serde_json::json!({
        "project_id": project_id,
        "docx_path": "",
        "markdown_path": "",
        "summary_path": null,
        "post_path": null,
        "published_channels": [],
        "created_at": chrono::Utc::now().to_rfc3339(),
        "channel_summaries": channel_summaries,
        "channel_posts": HashMap::<String, String>::new()
    });
    
    let metadata_path = project_dir.join("metadata.json");
    std::fs::write(metadata_path, serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    
    // Создаем extracted.md файл
    let markdown_text = "Тестовый markdown текст для проекта";
    let markdown_path = project_dir.join("extracted.md");
    std::fs::write(markdown_path, markdown_text).unwrap();
}

/// Рендерит конфигурацию с указанными каналами
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
    ctx.insert("rss_enabled", &false);
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


