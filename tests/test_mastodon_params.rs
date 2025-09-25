use luminis::run_with_config_path;
use serial_test::serial;
use wiremock::MockServer;
use urlencoding::decode;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon_with_params_check, mount_npalist, mount_stages,
    mount_telegram, read_mocks, render_config_with_mastodon_params,
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
async fn test_mastodon_visibility_public() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой visibility=public
    mount_mastodon_with_params_check(&server, Some("public"), None, None).await;

    // Setup config with public visibility
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        Some("public"), // mastodon_visibility
        None,  // mastodon_language (default)
        None,  // mastodon_sensitive (default)
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_visibility_private() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой visibility=private
    mount_mastodon_with_params_check(&server, Some("private"), None, None).await;

    // Setup config with private visibility
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        Some("private"), // mastodon_visibility
        None,  // mastodon_language (default)
        None,  // mastodon_sensitive (default)
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_language_en() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой language=en
    mount_mastodon_with_params_check(&server, None, Some("en"), None).await;

    // Setup config with English language
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        Some("en"), // mastodon_language
        None,  // mastodon_sensitive (default)
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_sensitive_true() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой sensitive=true
    mount_mastodon_with_params_check(&server, None, None, Some(true)).await;

    // Setup config with sensitive content
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        None,  // mastodon_language (default)
        Some(true), // mastodon_sensitive
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_sensitive_false() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой sensitive=false
    mount_mastodon_with_params_check(&server, None, None, Some(false)).await;

    // Setup config with non-sensitive content
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        None,  // mastodon_language (default)
        Some(false), // mastodon_sensitive
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_max_chars_custom() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon без проверки параметров (проверяем только вызов)
    mount_mastodon_with_params_check(&server, None, None, None).await;

    // Setup config with custom max_chars
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        None,  // mastodon_language (default)
        None,  // mastodon_sensitive (default)
        Some(1000), // mastodon_max_chars
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_language_de() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой language=de
    mount_mastodon_with_params_check(&server, None, Some("de"), None).await;

    // Setup config with German language
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        Some("de"), // mastodon_language
        None,  // mastodon_sensitive (default)
        None,  // mastodon_max_chars (default)
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_multiple_params() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем мок Mastodon с проверкой всех параметров
    mount_mastodon_with_params_check(&server, Some("direct"), Some("en"), Some(true)).await;

    // Setup config with multiple custom parameters
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        Some("direct"), // mastodon_visibility
        Some("en"), // mastodon_language
        Some(true), // mastodon_sensitive
        Some(2000), // mastodon_max_chars
    );

    // Run the application
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap(), None)
        .await
        .unwrap();

    // Verify that Mastodon was called with correct parameters
    server.verify().await;
}
