use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};
use mockito::{Server, Matcher, Mock};
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

pub async fn mount_rss(server: &mut Server, rss_xml: &str) -> Mock {
    server.mock("GET", "/api/public/Rss")
        .with_status(200)
        .with_body(rss_xml)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_rss_with_error(server: &mut Server) -> Mock {
    server.mock("GET", "/api/public/Rss")
        .with_status(500)
        .with_body("Internal Server Error")
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_npalist(server: &mut Server) -> Mock {
    let npalist_xml = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/npalist.xml"),
    )
    .unwrap();
    server.mock("GET", Matcher::Regex(r"/api/npalist/.*".to_string()))
        .with_status(200)
        .with_body(npalist_xml)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_npalist_with_error(server: &mut Server) -> Mock {
    server.mock("GET", "/api/npalist/?limit=50&offset=0&sort=desc")
        .with_status(500)
        .with_body("Internal Server Error")
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_stages(server: &mut Server, stages_json: &str) -> Mock {
    server.mock("GET", Matcher::Regex(r"/api/public/PublicProjects/GetProjectStages/\d+".to_string()))
        .with_status(200)
        .with_body(stages_json)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_docx(server: &mut Server) -> Mock {
    let docx_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/source.docx");
    
    // // Логирование содержимого DOCX через markdownify
    // match docx::docx_convert(&docx_path) {
    //     Ok(markdown) => {
    //         let preview = if markdown.chars().count() > 50 {
    //             format!("{}...", markdown.chars().take(50).collect::<String>())
    //         } else {
    //             markdown.clone()
    //         };
    //         println!("DOCX mock content: {}", preview);
    //     },
    //     Err(e) => {
    //         println!("DOCX mock markdownify error: {}", e);
    //     }
    // }
    
    server.mock("GET", Matcher::Regex(r"/api/public/Files/GetFile\?.*".to_string()))
        .with_status(200)
        .with_header(
            "content-type",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )
        .with_body_from_file(&docx_path)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_gemini_generate(server: &mut Server) -> Mock {
    let response_body = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "tests/resources/mocks/body-v1beta-models-gemini-2.0-flash_generateContent-8OOhY.json",
        ),
    )
    .unwrap();
    server.mock("POST", "/v1beta/models/gemini-2.0-flash:generateContent")
        .with_status(200)
        .with_header("content-type", "application/json; charset=UTF-8")
        .with_body(response_body)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_mastodon(server: &mut Server) -> Mock {
    let mstd_json = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mocks/mastodon_status.json"),
    )
    .unwrap();
    server.mock("POST", "/api/v1/statuses")
        .with_status(200)
        .with_body(mstd_json)
        .expect_at_least(1)
        .create_async().await
}

pub async fn mount_telegram(server: &mut Server) -> Mock {
    server.mock("POST", Matcher::Regex(r"/botTEST/sendMessage".to_string()))
        .with_status(200)
        .with_body("{\"ok\":true}")
        .expect_at_least(1)
        .create_async().await
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
pub async fn mount_gemini_generate_with_limit(server: &mut Server, limit: usize) -> Mock {
    let response_body = format!(
        r#"{{"candidates":[{{"content":{{"parts":[{{"text":"Краткая суммаризация для лимита {} символов. Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.\\n\\nРейтинг:\\nПолезность: 5/10 (частично улучшает ОМС)\\nРепрессивность: 2/10 (незначительно)\\nКоррупц. емкость: 6/10 (регион. перераспределение)"}}]}}}}]}}"#,
        limit
    );
    
    server.mock("POST", "/v1beta/models/gemini-2.0-flash:generateContent")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body)
        .expect_at_least(1)
        .create_async().await
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


