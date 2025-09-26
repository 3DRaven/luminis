use assert_fs::prelude::*;
use json_test::{JsonTest, PropertyAssertions};
use luminis::run_with_config_path;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use tokio;
use wiremock::MockServer;

mod common;

use common::{
    mount_docx, mount_gemini_generate, mount_npalist, mount_stages,
    mount_telegram, read_mocks, render_config,
};

#[tokio::test]
async fn test_manifest_and_metadata_persistence() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let cache = temp_dir.child("cache");
    let output_file = temp_dir.child("post.txt");
    
    // Создаем mock сервер
    let server = MockServer::start().await;
    let base = server.uri();
    
    // Мокаем API endpoints
    mount_npalist(&server).await;
    mount_stages(&server, &read_mocks()).await;
    mount_docx(&server).await;
    mount_gemini_generate(&server).await;
    mount_telegram(&server).await;
    
    // Создаем конфигурацию
    let cfg_file = render_config(
        &base,
        output_file.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        true,  // telegram_enabled
        true,  // console_enabled
        true,  // file_enabled
        true,  // npalist_enabled
    );
    
    // Запускаем приложение
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert_eq!(result.is_ok(), true, "Application should run successfully");
    
    // Проверяем, что manifest.json создан и содержит правильные данные
    let manifest_path = cache.child("manifest.json");
    manifest_path.assert(predicate::path::is_file());
    
    let manifest_content = fs::read_to_string(manifest_path.path()).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content).unwrap();
    
    // Проверяем структуру manifest.json с помощью json-test
    let mut manifest_test = JsonTest::new(&manifest);
    manifest_test
        .assert_path("$.min_published_project_id")
        .exists()
        .is_number()
        .is_greater_than(0);
    
    let min_published_id = manifest["min_published_project_id"].as_u64().unwrap();
    println!("✅ manifest.json содержит min_published_project_id: {}", min_published_id);
    
    // Проверяем, что metadata.json создан для обработанных проектов
    // Из мока npalist.xml знаем, что первый проект имеет ID 160532
    let project_id = "160532";
    let metadata_path = cache.child(project_id).child("metadata.json");
    metadata_path.assert(predicate::path::is_file());
    
    let metadata_content = fs::read_to_string(metadata_path.path()).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&metadata_content).unwrap();
    
    // Проверяем структуру metadata.json с помощью json-test
    // Основные поля
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.project_id")
            .exists()
            .is_string()
            .equals(json!(project_id));
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.docx_path")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.markdown_path")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.created_at")
            .exists()
            .is_string();
    }
    
    // Проверяем published_channels
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.published_channels")
            .exists()
            .is_array()
            .has_length(3)
            .contains(&json!("Telegram"))
            .contains(&json!("Console"))
            .contains(&json!("File"));
    }
    
    // Проверяем, что channel_summaries и channel_posts являются объектами
    {
        let mut test = JsonTest::new(&metadata);
        let _channel_summaries = test.assert_path("$.channel_summaries").exists().assert_object();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        let _channel_posts = test.assert_path("$.channel_posts").exists().assert_object();
    }
    
    // Проверяем, что для каждого канала есть суммаризация и пост
    // Используем JSONPath для проверки всех каналов сразу
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_summaries")
            .has_properties(vec!["Telegram", "Console", "File"]);
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_posts")
            .has_properties(vec!["Telegram", "Console", "File"]);
    }
    
    // Проверяем, что все значения являются строками
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_summaries.Telegram")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_summaries.Console")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_summaries.File")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_posts.Telegram")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_posts.Console")
            .exists()
            .is_string();
    }
    
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.channel_posts.File")
            .exists()
            .is_string();
    }
    
    // Проверяем, что crawl_metadata содержит метаданные из NpaListCrawler
    {
        let mut test = JsonTest::new(&metadata);
        test.assert_path("$.crawl_metadata")
            .exists()
            .is_array();
    }
    
    // Проверяем, что в crawl_metadata есть ожидаемые поля
    {
        let crawl_metadata = &metadata["crawl_metadata"];
        
        // Проверяем, что есть метаданные о дате, департаменте и ответственном
        {
            let mut crawl_test = JsonTest::new(crawl_metadata);
            crawl_test.assert_path("$[*].Date").exists();
        }
        
        {
            let mut crawl_test = JsonTest::new(crawl_metadata);
            crawl_test.assert_path("$[*].Department").exists();
        }
        
        {
            let mut crawl_test = JsonTest::new(crawl_metadata);
            crawl_test.assert_path("$[*].Responsible").exists();
        }
    }
    
    println!("✅ metadata.json для проекта {} содержит правильную структуру", project_id);
    println!("✅ channel_summaries и channel_posts заполнены для всех каналов");
    println!("✅ crawl_metadata содержит метаданные из NpaListCrawler");
    
    // Проверяем, что output файл содержит ожидаемый контент
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("ОМС"));
    
    println!("✅ Все проверки пройдены успешно!");
}
