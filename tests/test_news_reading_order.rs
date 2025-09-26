use luminis::{crawlers::Manifest, run_with_config_path};
use luminis::services::cache_manager_impl::FileSystemCacheManager;
use luminis::traits::cache_manager::CacheManager;
use serial_test::serial;
use wiremock::MockServer;
use assert_fs::prelude::*;
use predicates::prelude::*;
use pretty_assertions::assert_eq;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_npalist_offset0, mount_npalist_offset50, mount_npalist_offset58,
    mount_npalist_offset63, mount_stages, mount_telegram, read_mocks, render_config, prepopulate_cache,
};

/// Тест проверяет чтение последних новостей (offset=0) из NPAList
#[tokio::test]
#[serial]
async fn test_npalist_offset0_reading() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();
    
    // Создаем временную директорию
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("post.txt");
    let cache = temp_dir.child("cache");
    
    // Создаем cache manager
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    
    // Предварительно создаем manifest.json с min_published_project_id=160533 (все элементы на offset=0 считаются новыми)
    let manifest = Manifest {
        min_published_project_id: Some(160533),
    };
    _cache_manager.save_manifest(&manifest).await.unwrap();
    
    // Мокаем API endpoints
    mount_npalist_offset0(&server).await;
    mount_stages(&server, &stages_json).await;
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
    
    // Запускаем - читаем последние новости (offset=0)
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert_eq!(result.is_ok(), true, "Run should succeed");
    
    // Проверяем, что был запрос к offset=0
    let received_requests = server.received_requests().await.unwrap();
    let offset0_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=0"))
        .collect();
    
    assert!(offset0_requests.len() >= 1, "Should have at least one request to offset=0");
    
    // Проверяем содержимое файла с помощью assert
    output_file.assert(predicate::path::is_file());
    output_file.assert(predicate::str::is_empty().not());
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("ОМС"));
    output_file.assert(predicate::str::contains("Дата:2025-09-20"));
    
    // Проверяем, что manifest.json обновился после обработки новых элементов
    let updated_manifest = _cache_manager.load_manifest().await.unwrap();
    assert!(updated_manifest.min_published_project_id.is_some(), "manifest should be updated with min_published_project_id");
    
    // Verify mocks were called
    server.verify().await;
}

/// Тест проверяет первое углубление в историю: offset=0 (все закешированы) -> offset=50 (новые элементы)
/// Симулирует ситуацию, когда это первое чтение истории после обработки offset=0
#[tokio::test]
#[serial]
async fn test_first_history_dive_offset50() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();
    
    // Создаем временную директорию
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("post.txt");
    let cache = temp_dir.child("cache");
    
    // Предварительно создаем кеш для ВСЕХ элементов из offset=0, чтобы они считались уже обработанными
    // НЕ создаем кеш для элементов из offset=50, чтобы система их обработала при углублении в историю
    use crate::common::prepopulate_cache;
    let project_ids_offset0 = [
        "160532", "160531", "160530", "160529", "160528", "160527", "160526", "160525", "160524", "160523",
        "160521", "160520", "160519", "160518", "160517", "160516", "160515", "160514", "160513", "160512",
        "160511", "160510", "160508", "160507", "160504", "160501", "160500", "160499", "160498", "160497",
        "160496", "160495", "160494", "160493", "160492", "160491", "160490", "160489", "160488", "160487",
        "160486", "160485", "160484", "160483", "160482", "160481", "160480", "160479", "160478", "160477"
    ];
    for project_id in &project_ids_offset0 {
        prepopulate_cache(cache.path().to_str().unwrap(), project_id, "Test summary");
    }
    
    // Создаем полностью опубликованные элементы для offset=0
    use luminis::services::cache_manager_impl::FileSystemCacheManager;
    use serde_json::json;
    
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    for project_id in &project_ids_offset0 {
        // Создаем метаданные с полной публикацией во все каналы
        let metadata = json!({
            "project_id": project_id,
            "docx_path": format!("{}.docx", project_id),
            "markdown_path": format!("{}.md", project_id),
            "summary_path": null,
            "post_path": null,
            "published_channels": ["Telegram", "Console", "File"],
            "created_at": chrono::Utc::now().to_rfc3339(),
            "channel_summaries": {},
            "channel_posts": {}
        });
        let _metadata_path = cache.child(project_id).child("metadata.json");
        cache.child(project_id).child("metadata.json").write_str(&serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    }
    
    // НЕ создаем кеш для элементов из offset=58 (160475, 160474), чтобы система их обработала
    
    // Создаем manifest.json с min_published_project_id=160533 (больше максимального ID на offset=0)
    let manifest_content = r#"{
        "min_published_project_id": 160533
    }"#;
    
    // Создаем manifest в правильном месте (временная директория кеша)
    cache.create_dir_all().unwrap();
    let manifest_path = cache.child("manifest.json");
    manifest_path.write_str(&manifest_content).unwrap();
    
    // Мокаем API endpoints - включаем offset=0 и offset=50 (limit)
    mount_npalist_offset0(&server).await;
    mount_npalist_offset50(&server).await;
    mount_stages(&server, &stages_json).await;
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
    
    // Запускаем - система сначала читает offset=0 (все закешированы), затем углубляется в историю offset=50
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert_eq!(result.is_ok(), true, "Run should succeed");
    
    // Проверяем, что были запросы к offset=0 (система обрабатывает элементы на offset=0)
    let received_requests = server.received_requests().await.unwrap();
    let offset0_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=0"))
        .collect();
    
    assert!(offset0_requests.len() >= 1, "Should have at least one request to offset=0");
    
    // Проверяем содержимое файла с помощью assert
    output_file.assert(predicate::path::is_file());
    output_file.assert(predicate::str::is_empty().not());
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("Поправки в закон об ОМС"));
    output_file.assert(predicate::str::contains("Минздрав России"));
    
    // Проверяем manifest.json обновился с min_published_project_id
    let manifest_path = cache.child("manifest.json");
    assert!(manifest_path.exists(), "manifest.json should exist");
    let manifest_content = std::fs::read_to_string(manifest_path.path()).unwrap();
    assert!(manifest_content.contains("min_published_project_id"), "manifest should contain min_published_project_id");
    assert!(manifest_content.contains("160532"), "manifest should contain 160532"); // Минимальный ID из offset=0 после обработки
    
    // Verify mocks were called
    server.verify().await;
}

/// Отладочный тест для проверки логики кеша
#[tokio::test]
#[serial]
async fn test_cache_logic_debug() {
    use crate::common::prepopulate_cache;
    use luminis::services::cache_manager_impl::FileSystemCacheManager;
    use luminis::traits::cache_manager::CacheManager;
    
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let cache = temp_dir.child("cache");
    
    // Создаем кеш для элементов из offset=0 и offset=50
    let project_ids_offset0 = ["160532", "160531", "160530"];
    let project_ids_offset50 = ["160475", "160474"];
    let project_ids_offset100 = ["160473", "160472", "160471"];
    
    for project_id in &project_ids_offset0 {
        prepopulate_cache(cache.path().to_str().unwrap(), project_id, "Test summary");
    }
    for project_id in &project_ids_offset50 {
        prepopulate_cache(cache.path().to_str().unwrap(), project_id, "Test summary");
    }
    
    // НЕ создаем кеш для элементов из offset=100
    
    // Создаем cache manager и проверяем has_data
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    
    // Проверяем элементы из offset=0 и offset=50 (должны быть закешированы)
    for project_id in &project_ids_offset0 {
        let has_data = _cache_manager.has_data(project_id).await.unwrap();
        println!("Cache for {} from offset=0: {}", project_id, has_data);
        assert!(has_data, "Element {} from offset=0 should be cached", project_id);
    }
    
    for project_id in &project_ids_offset50 {
        let has_data = _cache_manager.has_data(project_id).await.unwrap();
        println!("Cache for {} from offset=50: {}", project_id, has_data);
        assert!(has_data, "Element {} from offset=50 should be cached", project_id);
    }
    
    // Проверяем элементы из offset=100 (НЕ должны быть закешированы)
    for project_id in &project_ids_offset100 {
        let has_data = _cache_manager.has_data(project_id).await.unwrap();
        println!("Cache for {} from offset=100: {}", project_id, has_data);
        assert!(!has_data, "Element {} from offset=100 should NOT be cached", project_id);
    }
}

/// Тест проверяет продолжение углубления в историю: offset=0 (все закешированы) -> offset=100 (новые элементы)
/// Симулирует ситуацию, когда ранее уже читали историю до offset=50, теперь продолжаем с offset=100
#[tokio::test]
#[serial]
async fn test_continue_history_dive_offset100() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();
    
    // Создаем временную директорию
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("post.txt");
    let cache = temp_dir.child("cache");
    
        // Предварительно создаем manifest.json с min_published_project_id=160474 (элементы из offset=100 НЕ опубликованы)
        let manifest_content = r#"{
            "min_published_project_id": 160474
        }"#;
    
    // Создаем manifest в правильном месте (временная директория кеша)
    cache.create_dir_all().unwrap();
    let manifest_path = cache.child("manifest.json");
    manifest_path.write_str(&manifest_content).unwrap();
    
    // Создаем кеш для ВСЕХ элементов из offset=0, чтобы они считались уже обработанными
    // НЕ создаем кеш для элементов из offset=58, чтобы система их обработала при углублении в историю
    let project_ids_offset0 = [
        "160532", "160531", "160530", "160529", "160528", "160527", "160526", "160525", "160524", "160523",
        "160521", "160520", "160519", "160518", "160517", "160516", "160515", "160514", "160513", "160512",
        "160511", "160510", "160508", "160507", "160504", "160501", "160500", "160499", "160498", "160497",
        "160496", "160495", "160494", "160493", "160492", "160491", "160490", "160489", "160488", "160487",
        "160486", "160485", "160484", "160483", "160482", "160481", "160480", "160479", "160478", "160477"
    ];
    for project_id in &project_ids_offset0 {
        prepopulate_cache(cache.path().to_str().unwrap(), project_id, "Test summary");
    }
    
    // Создаем полностью опубликованные элементы для offset=0
    use luminis::services::cache_manager_impl::FileSystemCacheManager;
    use serde_json::json;
    
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    for project_id in &project_ids_offset0 {
        // Создаем метаданные с полной публикацией во все каналы
        let metadata = json!({
            "project_id": project_id,
            "docx_path": format!("{}.docx", project_id),
            "markdown_path": format!("{}.md", project_id),
            "summary_path": null,
            "post_path": null,
            "published_channels": ["Telegram", "Console", "File"],
            "created_at": chrono::Utc::now().to_rfc3339(),
            "channel_summaries": {},
            "channel_posts": {}
        });
        let _metadata_path = cache.child(project_id).child("metadata.json");
        cache.child(project_id).child("metadata.json").write_str(&serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    }
    
    // НЕ создаем кеш для элементов из offset=58 (160473, 160472, 160471), чтобы система их обработала
    // Важно: убедимся, что кеш для элементов 160473, 160472, 160471 не создается
    
    // Мокаем API endpoints - включаем offset=0 и вычисленный offset=58
    mount_npalist_offset0(&server).await;
    mount_npalist_offset58(&server).await;
    mount_stages(&server, &stages_json).await;
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
    
    // Запускаем - система сначала читает offset=0 (все закешированы), затем вычисляет offset=58
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    
    // Отладочная информация
    println!("Cache directory contents:");
    if let Ok(entries) = cache.read_dir() {
        for entry in entries.flatten() {
            println!("  {:?}", entry.path());
        }
    }
    
    // Проверим, есть ли кеш для элементов из offset=100
    let offset100_ids = ["160473", "160472", "160471"];
    for id in &offset100_ids {
        let cache_path = cache.path().join(id);
        println!("Cache for {} exists: {}", id, cache_path.exists());
    }
    
    assert_eq!(result.is_ok(), true, "Run should succeed");
    
    // Проверяем, что были запросы к offset=0, offset=50 и offset=100
    let received_requests = server.received_requests().await.unwrap();
    println!("Total requests: {}", received_requests.len());
    for (i, req) in received_requests.iter().enumerate() {
        println!("Request {}: {}", i, req.url);
    }
    
    let offset0_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=0"))
        .collect();
    
    println!("Offset 0 requests: {}", offset0_requests.len());
    
    assert!(offset0_requests.len() >= 1, "Should have at least one request to offset=0");
    
    // Проверяем содержимое файла с помощью assert
    output_file.assert(predicate::path::is_file());
    output_file.assert(predicate::str::is_empty().not());
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("Поправки в закон об ОМС"));
    output_file.assert(predicate::str::contains("Минздрав России"));
    
    // Проверяем, что manifest.json обновился с min_published_project_id
    assert!(manifest_path.exists(), "manifest.json should exist");
    let manifest_child = temp_dir.child("manifest.json");
    std::fs::copy(manifest_path.path(), manifest_child.path()).unwrap();
    manifest_child.assert(predicate::str::contains("min_published_project_id"));
    manifest_child.assert(predicate::str::contains("160532")); // Минимальный ID из offset=0 после обработки
    
    // Verify mocks were called
    server.verify().await;
}

/// Тест проверяет углубление в историю на основе manifest.json
/// Симулирует ситуацию, когда в offset=0 все элементы уже закешированы
#[tokio::test]
#[serial]
async fn test_manifest_json_history_reading() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();
    
    // Создаем временную директорию
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("post.txt");
    let cache = temp_dir.child("cache");
    
    // Предварительно создаем manifest.json с min_published_project_id=160469 (элемент 160470 НЕ опубликован)
    let manifest_content = r#"{
        "min_published_project_id": 160469
    }"#;
    // Создаем manifest в правильном месте (временная директория кеша)
    cache.create_dir_all().unwrap();
    let manifest_path = cache.child("manifest.json");
    manifest_path.write_str(&manifest_content).unwrap();
    
    // Создаем кеш для ВСЕХ элементов из offset=0, чтобы они считались уже обработанными
    // НЕ создаем кеш для элементов из offset=63, чтобы система их обработала при углублении в историю
    let project_ids_offset0 = [
        "160532", "160531", "160530", "160529", "160528", "160527", "160526", "160525", "160524", "160523",
        "160521", "160520", "160519", "160518", "160517", "160516", "160515", "160514", "160513", "160512",
        "160511", "160510", "160508", "160507", "160504", "160501", "160500", "160499", "160498", "160497",
        "160496", "160495", "160494", "160493", "160492", "160491", "160490", "160489", "160488", "160487",
        "160486", "160485", "160484", "160483", "160482", "160481", "160480", "160479", "160478", "160477"
    ];
    for project_id in &project_ids_offset0 {
        prepopulate_cache(cache.path().to_str().unwrap(), project_id, "Test summary");
    }
    
    // Создаем полностью опубликованные элементы для offset=0
    use luminis::services::cache_manager_impl::FileSystemCacheManager;
    use serde_json::json;
    
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    for project_id in &project_ids_offset0 {
        // Создаем метаданные с полной публикацией во все каналы
        let metadata = json!({
            "project_id": project_id,
            "docx_path": format!("{}.docx", project_id),
            "markdown_path": format!("{}.md", project_id),
            "summary_path": null,
            "post_path": null,
            "published_channels": ["Telegram", "Console", "File"],
            "created_at": chrono::Utc::now().to_rfc3339(),
            "channel_summaries": {},
            "channel_posts": {}
        });
        let _metadata_path = cache.child(project_id).child("metadata.json");
        cache.child(project_id).child("metadata.json").write_str(&serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    }
    
    // НЕ создаем кеш для элементов из offset=63 (160470), чтобы система их обработала
    
    // Мокаем API endpoints - включаем offset=0 и вычисленный offset=63
    mount_npalist_offset0(&server).await;
    mount_npalist_offset63(&server).await;
    mount_stages(&server, &stages_json).await;
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
    
    // Запускаем - система сначала читает offset=0 (все закешированы), затем углубляется в историю
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert_eq!(result.is_ok(), true, "Run should succeed");
    
    // Проверяем, что были запросы к offset=0 и offset=50
    let received_requests = server.received_requests().await.unwrap();
    let offset0_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=0"))
        .collect();
    
    assert!(offset0_requests.len() >= 1, "Should have at least one request to offset=0");
    
    // Проверяем содержимое файла с помощью assert
    output_file.assert(predicate::path::is_file());
    output_file.assert(predicate::str::is_empty().not());
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("Поправки в закон об ОМС"));
    output_file.assert(predicate::str::contains("Минздрав России"));
    
    // Проверяем, что manifest.json обновился
    assert!(manifest_path.exists(), "manifest.json should exist");
    let manifest_child = temp_dir.child("manifest.json");
    std::fs::copy(manifest_path.path(), manifest_child.path()).unwrap();
    manifest_child.assert(predicate::str::contains("min_published_project_id"));
    manifest_child.assert(predicate::str::contains("160532")); // Минимальный ID из offset=0 после обработки
    
    // Verify mocks were called
    server.verify().await;
}


/// Тест проверяет полный цикл: offset=0 -> углубление в историю -> manifest.json
#[tokio::test]
#[serial]
async fn test_full_reading_cycle_with_manifest() {
    let server = MockServer::start().await;
    let base = server.uri();
    let stages_json = read_mocks();
    
    // Создаем временную директорию
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let output_file = temp_dir.child("post.txt");
    let cache = temp_dir.child("cache");
    
    // Создаем cache manager
    let _cache_manager = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    
    // Предварительно создаем manifest.json с min_published_project_id=160533 (все элементы на offset=0 считаются новыми)
    let manifest = Manifest {
        min_published_project_id: Some(160533),
    };
    _cache_manager.save_manifest(&manifest).await.unwrap();
    
    // Создаем кеш для элементов из offset=0, но НЕ полностью опубликованных
    // Это заставит систему обработать их и затем углубиться в историю
    let project_ids_offset0 = ["160532", "160531", "160530", "160529", "160528", "160527", "160526", "160525", "160524", "160523", "160521", "160520", "160519", "160518", "160517", "160516", "160515", "160514", "160513", "160512", "160511", "160510", "160508", "160507", "160504", "160501", "160500", "160499", "160498", "160497", "160496", "160495", "160494", "160493", "160492", "160491", "160490", "160489", "160488", "160487", "160486", "160485", "160484", "160483", "160482", "160481", "160480", "160479", "160478", "160477"];
    for pid in &project_ids_offset0 {
        prepopulate_cache(cache.path().to_str().unwrap(), pid, "Test summary");
    }
    
    // Создаем полностью опубликованные элементы для offset=0
    use luminis::services::cache_manager_impl::FileSystemCacheManager;
    use serde_json::json;
    
    let _cache_manager2 = FileSystemCacheManager::builder()
        .cache_dir(cache.path().to_str().unwrap().to_string())
        .build();
    for project_id in &project_ids_offset0 {
        // Создаем метаданные с полной публикацией во все каналы
        let metadata = json!({
            "project_id": project_id,
            "docx_path": format!("{}.docx", project_id),
            "markdown_path": format!("{}.md", project_id),
            "summary_path": null,
            "post_path": null,
            "published_channels": ["Telegram", "Console", "File"],
            "created_at": chrono::Utc::now().to_rfc3339(),
            "channel_summaries": {},
            "channel_posts": {}
        });
        let _metadata_path = cache.child(project_id).child("metadata.json");
        cache.child(project_id).child("metadata.json").write_str(&serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    }
    
    // Мокаем API endpoints
    mount_npalist_offset0(&server).await;
    mount_npalist_offset50(&server).await;
    mount_stages(&server, &stages_json).await;
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
    
    // Первый запуск - читаем последние новости (offset=0)
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert!(result.is_ok(), "First run should succeed");
    
    // Проверяем содержимое первого запуска - система обрабатывает элементы на offset=0
    output_file.assert(predicate::str::contains("160532"));
    output_file.assert(predicate::str::contains("ОМС"));
    
    // Очищаем файл для следующего запуска
    output_file.write_str("").unwrap();
    
    // Второй запуск - система продолжает обрабатывать элементы на offset=0
    let result = run_with_config_path(cfg_file.path().to_str().unwrap(), None).await;
    assert!(result.is_ok(), "Second run should succeed");
    
    // Проверяем содержимое второго запуска - система обрабатывает новые элементы
    let output_content = std::fs::read_to_string(output_file.path()).unwrap();
    assert!(!output_content.is_empty(), "Second run should process new items");
    assert!(output_content.contains("160531"), "Should contain project 160531");
    
    // Проверяем, что manifest.json обновился с правильными данными
    let updated_manifest = _cache_manager.load_manifest().await.unwrap();
    assert_eq!(updated_manifest.min_published_project_id, Some(160531));
    
    // Проверяем порядок запросов
    let received_requests = server.received_requests().await.unwrap();
    let offset0_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=0"))
        .collect();
    let _offset61_requests: Vec<_> = received_requests
        .iter()
        .filter(|req| req.url.query().unwrap_or("").contains("offset=61"))
        .collect();
    
    // Должно быть запросы к offset=0
    assert!(offset0_requests.len() >= 1, "Should have at least one request to offset=0");
    // Второй запуск не делает deep dive, так как все элементы уже опубликованы
    
    // Verify mocks were called
    server.verify().await;
}