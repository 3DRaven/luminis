use luminis::run_with_config_path;
use serial_test::serial;
use wiremock::MockServer;
use std::fs;
use wiremock::http::Method;
use urlencoding::decode;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist, mount_stages,
    mount_telegram, prepopulate_cache, read_mocks, render_config,
};

/// Проверяет содержимое Mastodon поста с декодированием URL-encoded данных
fn assert_mastodon_post_content(body: &[u8]) {
    let body_str = String::from_utf8_lossy(body);
    
    // Декодируем URL-encoded строку для более читаемых проверок
    let decoded_body = decode(&body_str).unwrap_or_else(|_| body_str.clone());
    
    // Логируем декодированное тело запроса для отладки
    println!("Mastodon decoded body: {}", decoded_body);
    
    // Проверяем декодированное содержимое
    assert!(decoded_body.contains("regulation.gov.ru/projects/160532"), "Mastodon post should contain URL");
    assert!(decoded_body.contains("Поправки"), "Mastodon post should contain summary");
    assert!(decoded_body.contains("Рейтинг"), "Mastodon post should contain rating");
    assert!(decoded_body.contains("Метаданные"), "Mastodon post should contain metadata");
    
    // Проверяем конкретный текст из ответа Gemini (используем декодированную строку)
    // Mastodon использует лимит 495 символов, поэтому текст отличается от других каналов
    assert!(decoded_body.contains("Поправки+в+закон+об+ОМС"), "Mastodon post should contain Gemini summary text");
    assert!(decoded_body.contains("Губернаторы+смогут+передавать"), "Mastodon post should contain Gemini text about governors");
    assert!(decoded_body.contains("Полезность:+5/10"), "Mastodon post should contain Gemini rating");
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_mastodon(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Проверка содержимого файла
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    assert_eq!(out, expected_content, "File content should match expected output");

    // Детальная проверка публикации в Mastodon
    let received_requests = server.received_requests().await.unwrap();
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();
    
    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");
    
    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);
    
    // Проверяем содержимое поста в Mastodon
    let body_str = String::from_utf8_lossy(&mastodon_request.body);
    
    // Декодируем URL-encoded строку для более читаемых проверок
    let decoded_body = decode(&body_str).unwrap_or_else(|_| body_str.clone());
    
    // Логируем декодированное тело запроса для отладки
    println!("Mastodon decoded body: {}", decoded_body);
    
    // Проверяем конкретный текст из ответа Gemini (используем декодированную строку)
    // Mastodon использует лимит 495 символов, поэтому текст отличается от других каналов
    assert!(decoded_body.contains("Поправки+в+закон+об+ОМС"), "Mastodon post should contain Gemini summary text");
    assert!(decoded_body.contains("Губернаторы+смогут+передавать"), "Mastodon post should contain Gemini text about governors");
    assert!(decoded_body.contains("Полезность:+5/10"), "Mastodon post should contain Gemini rating");
    assert!(decoded_body.contains("regulation.gov.ru/projects/160532"), "Mastodon post should contain URL");
    assert!(decoded_body.contains("Метаданные"), "Mastodon post should contain metadata");

    // Verify mocks were called
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_only_file_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        false, // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    assert_eq!(out, expected_content, "File content should match expected output");

    // Verify mocks were called (cache: stages/gemini not necessarily called)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_console_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Детальная проверка публикации в Telegram
    let received_requests = server.received_requests().await.unwrap();
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let body_str = String::from_utf8_lossy(&telegram_request.body);
        assert!(body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Verify mocks were called (no cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_console_and_file_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    assert_eq!(out, expected_content, "File content should match expected output");

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_telegram_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    mount_mastodon(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

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
           assert!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(telegram_body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(telegram_body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(telegram_body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Проверка Mastodon
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();

    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");

    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);

    // Проверяем содержимое поста в Mastodon
    assert_mastodon_post_content(&mastodon_request.body);

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Детальная проверка публикации в Telegram
    let received_requests = server.received_requests().await.unwrap();
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let telegram_body_str = String::from_utf8_lossy(&telegram_request.body);
           assert!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(telegram_body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(telegram_body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(telegram_body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Verify mocks were called (no cache)
    server.verify().await;
}

// Дополнительные тесты для полного покрытия NPAList сценариев

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]
";
    assert_eq!(
        out.trim(),
        expected_content.trim(),
        "output file content must match expected post template"
    );

    // Детальная проверка публикации в Telegram
    let received_requests = server.received_requests().await.unwrap();
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let telegram_body_str = String::from_utf8_lossy(&telegram_request.body);
           assert!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(telegram_body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(telegram_body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(telegram_body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Verify mocks were called (no cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    assert_eq!(out, expected_content, "File content should match expected output");

    // Детальная проверка публикации в Telegram
    let received_requests = server.received_requests().await.unwrap();
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let telegram_body_str = String::from_utf8_lossy(&telegram_request.body);
           assert!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(telegram_body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(telegram_body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(telegram_body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_console_from_npalist_without_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_mastodon(&server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Детальная проверка публикации в Mastodon
    let received_requests = server.received_requests().await.unwrap();
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();

    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");

    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);

    // Проверяем содержимое поста в Mastodon
    assert_mastodon_post_content(&mastodon_request.body);

    // Verify mocks were called (no cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_console_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_mastodon(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Детальная проверка публикации в Mastodon
    let received_requests = server.received_requests().await.unwrap();
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();

    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");

    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);

    // Проверяем содержимое поста в Mastodon
    assert_mastodon_post_content(&mastodon_request.body);

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_mastodon(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );
    
    // Проверка полного содержимого файла
    let expected_content = "https://regulation.gov.ru/projects/160532
Поправки в закон об ОМС: Губернаторы смогут передавать полномочия страховых компаний тер. фондам ОМС (с ограничениями), уточнен статус иностр. граждан. Льготы работникам фед. фонда ОМС. Финансирование мед.помощи в новых регионах.

Рейтинг:
Полезность: 5/10 (частично улучшает ОМС)
Репрессивность: 2/10 (незначительно)
Коррупц. емкость: 6/10 (регион. перераспределение)

Метаданные: [Деп:Минздрав России; Отв:Филиппов Олег Анатольевич]

";
    assert_eq!(out, expected_content, "File content should match expected output");

    // Детальная проверка публикации в Mastodon
    let received_requests = server.received_requests().await.unwrap();
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();

    assert_eq!(mastodon_requests.len(), 1, "Should have exactly one Mastodon post");

    let mastodon_request = &mastodon_requests[0];
    assert_eq!(mastodon_request.method, Method::POST);

    // Проверяем содержимое поста в Mastodon
    assert_mastodon_post_content(&mastodon_request.body);

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_npalist_with_cache() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160531",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Детальная проверка публикации в Telegram
    let received_requests = server.received_requests().await.unwrap();
    let telegram_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path().contains("sendMessage"))
        .collect();
    
    assert_eq!(telegram_requests.len(), 1, "Should have exactly one Telegram post");
    
    let telegram_request = &telegram_requests[0];
    assert_eq!(telegram_request.method, Method::POST);
    
    // Проверяем содержимое поста в Telegram
    let telegram_body_str = String::from_utf8_lossy(&telegram_request.body);
           assert!(telegram_body_str.contains("https://regulation.gov.ru/projects/160532"), "Telegram post should contain URL");
    assert!(telegram_body_str.contains("Поправки в закон об ОМС"), "Telegram post should contain summary");
    assert!(telegram_body_str.contains("Рейтинг:"), "Telegram post should contain rating");
    assert!(telegram_body_str.contains("Метаданные:"), "Telegram post should contain metadata");

    // Verify mocks were called (stages/gemini skipped due to cache)
    server.verify().await;
}
