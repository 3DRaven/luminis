use luminis::run_with_config_path;
use luminis::services::chat_api::ChatApi;
use luminis::services::documents::DocumentFetcher;
use luminis::services::documents::save_cache_artifacts;
use rstest::rstest;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tera::{Context, Tera};
use wiremock::matchers::{body_json, header, method, path, path_regex};
use wiremock::{Mock, MockServer, MockServerBuilder, ResponseTemplate};

fn read_mocks() -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rss = fs::read_to_string(root.join("tests/mocks/rss.xml")).unwrap();
    let stages = fs::read_to_string(root.join("tests/mocks/stages.json")).unwrap();
    (rss, stages)
}

fn load_test_config_template() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/configs/mastodon_telegram.yaml");
    fs::read_to_string(p).unwrap()
}

#[rstest]
#[case(true, true, false, true, true, false, true)] // M+T, file, rss on, npalist off, with cache
#[case(true, false, false, true, false, true, false)] // only M, file, rss off, npalist on, no cache
#[case(false, true, false, true, true, false, true)] // only T, file, rss on, npalist off, with cache
#[case(false, false, true, false, true, false, false)] // only console, rss on, no cache
#[case(false, false, false, true, false, true, true)] // only file, npalist on, with cache
#[case(true, true, true, true, true, true, false)] // all publishers, both sources, no cache
#[tokio::test]
async fn publish_parametrized(
    #[case] mastodon: bool,
    #[case] telegram: bool,
    #[case] console: bool,
    #[case] file: bool,
    #[case] rss_enabled: bool,
    #[case] npalist_enabled: bool,
    #[case] with_cache: bool,
) {
    // Start mock server
    let server = MockServer::builder().start().await;
    let base = server.uri();

    // Load fixtures
    let (rss_xml, stages_json) = read_mocks();

    // Mock RSS (conditionally)
    if rss_enabled {
        Mock::given(method("GET"))
            .and(path("/api/public/Rss"))
            .respond_with(ResponseTemplate::new(200).set_body_string(rss_xml.clone()))
            .mount(&server)
            .await;
    }
    // Mock NpaList (conditionally)
    if npalist_enabled {
        let npalist_xml = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/npalist.xml"),
        )
        .unwrap();
        Mock::given(method("GET"))
            .and(path_regex(r"/api/npalist/\?limit=\d+&offset=\d+&sort=desc"))
            .respond_with(ResponseTemplate::new(200).set_body_string(npalist_xml))
            .mount(&server)
            .await;
    }

    // If будем публиковаться без кэша — смокать скачивание DOCX
    if !with_cache {
        let docx_bytes =
            fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/source.docx"))
                .unwrap();
        Mock::given(method("GET"))
            .and(path("/api/public/Files/GetFile"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header(
                        "content-type",
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    )
                    .set_body_bytes(docx_bytes),
            )
            .mount(&server)
            .await;
    }
    // Mock stages (fileId)
    Mock::given(method("GET"))
        .and(path_regex(
            r"/api/public/PublicProjects/GetProjectStages/\d+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(stages_json))
        .mount(&server)
        .await;
    // We will avoid DOCX download by pre-populating cache summary
    // Mock Telegram sendMessage (optional)
    let _tg_guard = if telegram {
        Some(
            Mock::given(method("POST"))
                .and(path_regex(r"/botTEST/sendMessage"))
                .respond_with(ResponseTemplate::new(200).set_body_string("{\"ok\":true}"))
                .expect(1)
                .named("telegram POST /sendMessage")
                .mount(&server)
                .await,
        )
    } else {
        None
    };
    // Mock Mastodon statuses (optional)
    let _mstd_guard = if mastodon {
        let mstd_json = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/mastodon_status.json"),
        )
        .unwrap();
        Some(
            Mock::given(method("POST"))
                .and(path("/api/v1/statuses"))
                .respond_with(ResponseTemplate::new(200).set_body_string(mstd_json))
                .expect(1)
                .named("mastodon POST /api/v1/statuses")
                .mount(&server)
                .await,
        )
    } else {
        None
    };

    // Prepare temp output file and optional cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    if with_cache {
        // Prepopulate cache for project 160532 (first in RSS)
        save_cache_artifacts(
            cache.path().to_str().unwrap(),
            "160532",
            None,
            "",
            "Краткая суммаризация",
            "",
            &[],
        )
        .unwrap();
    }

    // Write temp config from Tera template with flags
    let tpl = load_test_config_template();
    let mut tera = Tera::default();
    tera.add_raw_template("cfg", &tpl).unwrap();
    let mut ctx = Context::new();
    ctx.insert("base", &base);
    ctx.insert("out", tf.path().to_str().unwrap());
    ctx.insert("cache", cache.path().to_str().unwrap());
    ctx.insert("mastodon_enabled", &mastodon);
    ctx.insert("telegram_enabled", &telegram);
    ctx.insert("console_enabled", &console);
    ctx.insert("file_enabled", &file);
    ctx.insert("rss_enabled", &rss_enabled);
    ctx.insert("npalist_enabled", &npalist_enabled);
    // Gemini generateContent: матчинг по ключевым частям prompt (совместимо с wiremock-rs)
    let response_body = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/mocks/body-v1beta-models-gemini-2.0-flash_generateContent-8OOhY.json"),
    )
    .unwrap();
    let mut gem_mock = Mock::given(method("POST"))
        .and(path_regex(".*generateContent.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json; charset=UTF-8")
                .set_body_string(response_body),
        )
        .named(format!("Gemini Generate Content contains {}", base));
    // Ожидать вызов только если нет кэша
    // if !with_cache {
    //     gem_mock = gem_mock.expect(1..);
    // }
    let _gem_mock_gen = gem_mock.mount(&server).await;

    // Force LLM to Gemini pointing to mock server
    ctx.insert("llm_model", &"gemini-2.0-flash");
    ctx.insert("llm_provider", &"Gemini");
    let base_llm = format!("{}/v1beta", base);
    ctx.insert("llm_base_url", &base_llm);
    ctx.insert("llm_api_key", &"TESTKEY");
    let config_text = tera.render("cfg", &ctx).unwrap();
    let cfg_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(cfg_file.path(), config_text).unwrap();
    println!(
        "TEST CONFIG PATH: {}",
        cfg_file.path().to_str().unwrap()
    );
    println!(
        "TEST CONFIG CONTENT:\n{}",
        fs::read_to_string(cfg_file.path()).unwrap()
    );

    // Run app once
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Assert file if enabled
    if file {
        let out = fs::read_to_string(tf.path()).unwrap();
        assert!(
            !out.trim().is_empty(),
            "output file must contain published post"
        );
    }
    // Verify all expectations on server (telegram/mastodon/gemini as applicable)
    server.verify().await;
}

#[tokio::test]
async fn fetch_docx_via_wiremock() {
    let server = MockServer::start().await;
    let base = server.uri();

    // Mock stages to return a known fileId
    let stages = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/stages.json"),
    )
    .unwrap();
    Mock::given(method("GET"))
        .and(path_regex(
            r"/api/public/PublicProjects/GetProjectStages/\d+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(stages))
        .mount(&server)
        .await;

    // Mock file download to return a real DOCX from mocks
    let docx_bytes =
        fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/source.docx"))
            .unwrap();
    Mock::given(method("GET"))
        .and(path("/api/public/Files/GetFile"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "content-type",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                )
                .set_body_bytes(docx_bytes),
        )
        .mount(&server)
        .await;

    // Call DocumentFetcher directly
    let fetcher = DocumentFetcher::new(Some(format!(
        "{}/api/public/PublicProjects/GetProjectStages/{{project_id}}",
        base
    )));
    let res = fetcher.fetch_docx("160532").await.unwrap();
    assert!(res.is_some(), "DOCX should be fetched and parsed");
    let (_bytes, md) = res.unwrap();
    assert!(
        !md.trim().is_empty(),
        "Extracted markdown should not be empty"
    );
}

#[tokio::test]
async fn test_gemini_api_client() {
    let mock_server = MockServer::start().await;
    let base = format!("{}/v1beta", mock_server.uri());

    let prompt_text = "Write a story about a magic backpack.";

    // Mock Gemini generateContent with strict body/headers match and expectation
    let _gemini_guard = Mock::given(method("POST"))
        .and(path_regex(r".*/:generateContent$"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(json!({
                    "candidates": [
                        {
                            "content": {
                                "parts": [
                                    {"text": "Maya discovered the backpack at a dusty antique shop..."}
                                ],
                                "role": "model"
                            },
                            "finishReason": "STOP",
                            "index": 0
                        }
                    ],
                    "usageMetadata": {
                        "promptTokenCount": 8,
                        "candidatesTokenCount": 312,
                        "totalTokenCount": 320
                    }
                }))
        )
        .expect(1)
        .named("Gemini Generate Content")
        .mount(&mock_server)
        .await;

    // Build LocalChatApi client pointing to mock Gemini
    let llm = luminis::services::settings::LlmConfig {
        model: Some("gemini-2.0-flash".to_string()),
        use_local: None,
        model_path: None,
        tokenizer_path: None,
        variant: None,
        temperature: None,
        top_p: None,
        max_new_tokens: None,
        seed: None,
        sliding_window: None,
        prompt_compression_ratio: None,
        enable_prompt_cache: None,
        enable_similarity_index: None,
        minhash_num_bands: None,
        minhash_band_width: None,
        minhash_jaccard_threshold: None,
        provider: Some("Gemini".to_string()),
        base_url: Some(base.clone()),
        proxy: None,
        api_key: Some("TESTKEY".to_string()),
        request_timeout_secs: Some(10),
        log_prompt_preview_chars: Some(40),
    };
    let api = luminis::services::chat_api_local::LocalChatApi::from_config(&llm);
    let resp = api
        .call_chat_api(prompt_text)
        .await
        .expect("gemini call ok");
    assert!(resp.contains("Maya discovered the backpack"));
    mock_server.verify().await;
}
