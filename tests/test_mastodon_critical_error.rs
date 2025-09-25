use luminis::run_with_config_path;
use serial_test::serial;
use wiremock::MockServer;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_mastodon_app_registration,
    mount_mastodon_token_exchange, mount_npalist, mount_stages,
    mount_telegram, read_mocks, render_config_with_mastodon_params,
};
use wiremock::http::Method;
use urlencoding::decode;
// use assert_cmd::Command; // Not used in current implementation

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
async fn test_mastodon_critical_error_no_token_no_login_cli() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config with Mastodon enabled but no token and login_cli=false
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
        None,  // mastodon_max_chars (default)
    );

    // Modify config to set login_cli=false and remove access_token
    let mut config_content = std::fs::read_to_string(cfg_file.path()).unwrap();
    config_content = config_content.replace("login_cli: true", "login_cli: false");
    config_content = config_content.replace("access_token: TEST", "access_token: \"\"");
    std::fs::write(cfg_file.path(), config_content).unwrap();

    // Run the application - should fail with critical error
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    
    // Verify that the application failed with the expected error
    assert!(result.is_err(), "Application should fail when Mastodon is enabled but no token available and login_cli=false");
    
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    println!("Actual error message: {}", error_msg);
    
    // Check for the shutdown error (which indicates a subsystem failed)
    assert!(error_msg.contains("shutdown error"), 
        "Error message should contain 'shutdown error', got: {}", error_msg);
    
    // The error indicates that at least one subsystem returned an error
    assert!(error_msg.contains("at least one subsystem returned an error"), 
        "Error message should indicate subsystem failure, got: {}", error_msg);
}

#[tokio::test]
#[serial]
async fn test_mastodon_works_with_valid_token() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    mount_mastodon(&server).await;

    // Setup config with Mastodon enabled and valid token
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
        None,  // mastodon_max_chars (default)
    );

    // Modify config to set login_cli=false but keep valid access_token
    let mut config_content = std::fs::read_to_string(cfg_file.path()).unwrap();
    config_content = config_content.replace("login_cli: true", "login_cli: false");
    // Keep access_token: TEST (valid token)
    std::fs::write(cfg_file.path(), config_content).unwrap();

    // Run the application - should work fine
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    
    // Verify that the application succeeded
    assert!(result.is_ok(), "Application should work when Mastodon is enabled with valid token: {:?}", result.err());

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

    // Verify mocks were called
    server.verify().await;
}

#[tokio::test]
#[serial]
async fn test_mastodon_disabled_no_error() {
    let server = MockServer::start().await;
    let base = server.uri();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks
    mount_npalist(&server).await;
    mount_stages(&server, &stages_json).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;

    // Setup config with Mastodon disabled
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_mastodon_params(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled = false
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        None,  // mastodon_visibility (default)
        None,  // mastodon_language (default)
        None,  // mastodon_sensitive (default)
        None,  // mastodon_max_chars (default)
    );

    // Run the application - should work fine even without token
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    
    // Verify that the application succeeded
    assert!(result.is_ok(), "Application should work when Mastodon is disabled: {:?}", result.err());

    // Проверяем, что Mastodon НЕ был вызван (поскольку он отключен)
    let received_requests = server.received_requests().await.unwrap();
    let mastodon_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.path() == "/api/v1/statuses")
        .collect();
    
    assert_eq!(mastodon_requests.len(), 0, "Should have no Mastodon posts when disabled");

    // Verify mocks were called
    server.verify().await;
}