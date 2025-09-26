use luminis::{models::config::LlmConfig, run_with_config_path};
use luminis::services::documents::DocxMarkdownFetcher;
use luminis::traits::chat_api::ChatApi;
use luminis::traits::markdown_fetcher::MarkdownFetcher;
use serial_test::serial;
use wiremock::MockServer;
use assert_fs::prelude::*;
use predicates::prelude::*;
use wiremock::http::Method;
use urlencoding::decode;
use pretty_assertions::assert_eq;

mod common;

use common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist, mount_stages,
    mount_telegram, read_mocks, render_config,
};


#[tokio::test]
#[serial]
async fn publish_all_publishers_from_both_sources_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    mount_mastodon(&server).await;

    // Setup config without cache
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("output.txt");
    let cache = temp_dir.child("cache");

    let cfg_file = render_config(
        &base,
        output_file.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true, // mastodon_enabled
        true, // telegram_enabled
        true, // console_enabled
        true, // file_enabled
        true, // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    output_file.assert(predicate::str::is_empty().not());
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Дата:2025-09-20; Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    output_file.assert(expected_content);

    // Детальная проверка публикации в Telegram и Mastodon
    let received_requests = server.received_requests().await.unwrap();
    
    // Проверка Telegram
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let telegram_body_str = String::from_utf8_lossy(&telegram_request.body);
    assert_eq!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), true, "Telegram post should contain URL");
    assert_eq!(telegram_body_str.contains("Поправки в закон об ОМС"), true, "Telegram post should contain summary");
    assert_eq!(telegram_body_str.contains("Рейтинг:"), true, "Telegram post should contain rating");
    assert_eq!(telegram_body_str.contains("Метаданные:"), true, "Telegram post should contain metadata");
    
    // Проверка Mastodon
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();
    
    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");
    
    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);
    
    // Проверяем содержимое поста в Mastodon
    let mastodon_body_str = String::from_utf8_lossy(&mastodon_request.body);
    
    // Декодируем URL-encoded строку для более читаемых проверок
    let decoded_body = decode(&mastodon_body_str).unwrap_or_else(|_| mastodon_body_str.clone());
    
    // Логируем декодированное тело запроса для отладки
    println!("Mastodon decoded body: {}", decoded_body);
    
    // Проверяем конкретный текст из ответа Gemini (используем декодированную строку)
    // Mastodon использует лимит 495 символов, поэтому текст отличается от других каналов
    assert_eq!(decoded_body.contains("Поправки+в+закон+об+ОМС"), true, "Mastodon post should contain Gemini summary text");
    assert_eq!(decoded_body.contains("Губернаторы+смогут+передавать"), true, "Mastodon post should contain Gemini text about governors");
    assert_eq!(decoded_body.contains("Полезность:+5/10"), true, "Mastodon post should contain Gemini rating");
    assert_eq!(decoded_body.contains("regulation.gov.ru/projects/160532"), true, "Mastodon post should contain URL");
    assert_eq!(decoded_body.contains("Метаданные"), true, "Mastodon post should contain metadata");

    // Verify mocks were called (no cache).
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn fetch_docx_via_wiremock() {
    let server = MockServer::start().await;
    let base = server.uri();

    // Setup mocks using test_utils
    let stages_json = read_mocks();
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    // Call MarkdownFetcher (Docx implementation) directly
    let template = format!(
        "{}/api/public/PublicProjects/GetProjectStages/{{project_id}}",
        base
    );
    let fetcher = DocxMarkdownFetcher::builder()
        .file_id_url_template(template)
        .build();
    let res = fetcher.fetch_markdown("160532").await.unwrap();
    assert_eq!(res.is_some(), true, "DOCX should be fetched and parsed");
    let (_bytes, md) = res.unwrap();
    assert_eq!(md.trim().is_empty(), false, "Extracted markdown should not be empty");
      // Verify mocks were called
      server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_gemini_api_client() {
    let mut mock_server = MockServer::start().await;
    let base = format!("{}/v1beta", mock_server.uri());

    let prompt_text = "Write a story about a magic backpack.";

    // Setup Gemini mock using test_utils
    mount_gemini_generate(&mut mock_server).await;

    // Build LocalChatApi client pointing to mock Gemini
    let llm = LlmConfig {
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
        max_retry_attempts: Some(3),
        retry_delay_secs: Some(2),
        log_prompt_preview_chars: Some(40),
    };
    let api = luminis::services::chat_api_local::LocalChatApi::from_config(&llm);
    let resp = api
        .call_chat_api(prompt_text)
        .await
        .expect("gemini call ok");
    assert_eq!(resp.contains("Поправки в закон об ОМС"), true);
}
