use luminis::run_with_config_path;
use serial_test::serial;
use wiremock::MockServer;
use std::fs;
use wiremock::http::Method;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist, mount_stages,
    mount_telegram, prepopulate_cache, read_mocks, render_config,
};

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
        assert!(body_str.contains("regulation.gov.ru%2Fprojects%2F160532"), "Mastodon post should contain URL");
    assert!(body_str.contains("%D0%9F%D0%BE%D0%BF%D1%80%D0%B0%D0%B2%D0%BA%D0%B8"), "Mastodon post should contain summary");
    assert!(body_str.contains("%D0%A0%D0%B5%D0%B9%D1%82%D0%B8%D0%BD%D0%B3"), "Mastodon post should contain rating");
    assert!(body_str.contains("%D0%9C%D0%B5%D1%82%D0%B0%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5"), "Mastodon post should contain metadata");

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
    let mastodon_body_str = String::from_utf8_lossy(&mastodon_request.body);
           assert!(mastodon_body_str.contains("regulation.gov.ru%2Fprojects%2F160532"), "Mastodon post should contain URL");
    assert!(mastodon_body_str.contains("%D0%9F%D0%BE%D0%BF%D1%80%D0%B0%D0%B2%D0%BA%D0%B8"), "Mastodon post should contain summary");
    assert!(mastodon_body_str.contains("%D0%A0%D0%B5%D0%B9%D1%82%D0%B8%D0%BD%D0%B3"), "Mastodon post should contain rating");
    assert!(mastodon_body_str.contains("%D0%9C%D0%B5%D1%82%D0%B0%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5"), "Mastodon post should contain metadata");

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
    let mastodon_body_str = String::from_utf8_lossy(&mastodon_request.body);
           assert!(mastodon_body_str.contains("regulation.gov.ru%2Fprojects%2F160532"), "Mastodon post should contain URL");
    assert!(mastodon_body_str.contains("%D0%9F%D0%BE%D0%BF%D1%80%D0%B0%D0%B2%D0%BA%D0%B8"), "Mastodon post should contain summary");
    assert!(mastodon_body_str.contains("%D0%A0%D0%B5%D0%B9%D1%82%D0%B8%D0%BD%D0%B3"), "Mastodon post should contain rating");
    assert!(mastodon_body_str.contains("%D0%9C%D0%B5%D1%82%D0%B0%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5"), "Mastodon post should contain metadata");

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
    let mastodon_body_str = String::from_utf8_lossy(&mastodon_request.body);
           assert!(mastodon_body_str.contains("regulation.gov.ru%2Fprojects%2F160532"), "Mastodon post should contain URL");
    assert!(mastodon_body_str.contains("%D0%9F%D0%BE%D0%BF%D1%80%D0%B0%D0%B2%D0%BA%D0%B8"), "Mastodon post should contain summary");
    assert!(mastodon_body_str.contains("%D0%A0%D0%B5%D0%B9%D1%82%D0%B8%D0%BD%D0%B3"), "Mastodon post should contain rating");
    assert!(mastodon_body_str.contains("%D0%9C%D0%B5%D1%82%D0%B0%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5"), "Mastodon post should contain metadata");

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
    let mastodon_body_str = String::from_utf8_lossy(&mastodon_request.body);
           assert!(mastodon_body_str.contains("regulation.gov.ru%2Fprojects%2F160532"), "Mastodon post should contain URL");
    assert!(mastodon_body_str.contains("%D0%9F%D0%BE%D0%BF%D1%80%D0%B0%D0%B2%D0%BA%D0%B8"), "Mastodon post should contain summary");
    assert!(mastodon_body_str.contains("%D0%A0%D0%B5%D0%B9%D1%82%D0%B8%D0%BD%D0%B3"), "Mastodon post should contain rating");
    assert!(mastodon_body_str.contains("%D0%9C%D0%B5%D1%82%D0%B0%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5"), "Mastodon post should contain metadata");

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

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
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
