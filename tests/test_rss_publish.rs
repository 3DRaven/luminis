use luminis::run_with_config_path;
use serial_test::serial;
use mockito::Server;
use std::fs;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist_with_error, mount_rss, mount_rss_with_error, mount_stages,
    mount_telegram, prepopulate_cache, read_mocks, render_config, render_config_with_retry_limit,
};

#[tokio::test]
#[serial]
async fn publish_mastodon_and_telegram_and_file_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPA error -> RSS fallback + Stages + Gemini + Telegram + Mastodon
    let _mock_npa_error = mount_npalist_with_error(&mut server).await;
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (stages and gemini not called because cache is used)
    // mock_npa_error.assert_async().await; // NPA краулер падает с ошибкой
    mock_rss.assert_async().await; // RSS fallback срабатывает
    // mock_stages не вызывается из-за кэша
    // mock_gemini не вызывается из-за кэша
    mock_telegram.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + Gemini + Telegram
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (stages and gemini not called because cache is used)
    mock_rss.assert_async().await;
    // _mock_stages не вызывается из-за кэша
    // _mock_gemini не вызывается из-за кэша
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_console_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;

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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (without cache all mocks should be called)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_full_no_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();

    // Setup mocks using test_utils
    let (rss_xml, stages_json) = read_mocks();
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup Gemini mock using test_utils
    let mock_gemini = mount_gemini_generate(&mut server).await;

    // Build config from template (no cache)
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    // Act: run app with generated config (single run)
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Assert: output file contains summary
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        out.contains("Поправки в закон об ОМС"),
        "post must contain summary"
    );
    
    // Verify expectations on server - all mocks should be called without cache
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Mastodon
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

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
        false, // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Telegram
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;

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
        false, // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_only_file_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        false, // telegram_enabled
        false, // console_enabled
        true,  // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_console_and_file_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + Gemini
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_rss.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_console_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + Gemini + Mastodon
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_rss.assert_async().await;
    mock_mastodon.assert_async().await;
}

// Дополнительные тесты для полного покрытия RSS сценариев

#[tokio::test]
#[serial]
async fn publish_mastodon_and_telegram_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Telegram + Mastodon
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup config without cache
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        false, // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_telegram_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPA error -> RSS fallback + Stages + Gemini + Telegram + Mastodon
    let _mock_npa_error = mount_npalist_with_error(&mut server).await;
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
        "Краткая суммаризация",
    );

    let cfg_file = render_config(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        true,  // mastodon_enabled
        true,  // telegram_enabled
        false, // console_enabled
        false, // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_rss.assert_async().await;
    mock_telegram.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Mastodon
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + Gemini + Mastodon
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_mastodon = mount_mastodon(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_rss.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Telegram
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;

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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_rss_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + DOCX + Gemini + Telegram
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    let mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;

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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_rss.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_rss_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + Stages + Gemini + Telegram
    let mock_rss = mount_rss(&mut server, &rss_xml).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_gemini = mount_gemini_generate(&mut server).await;
    let mock_telegram = mount_telegram(&mut server).await;

    // Setup config with cache prepopulated
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    prepopulate_cache(
        cache.path().to_str().unwrap(),
        "160532",
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой, сработает RSS fallback)
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_rss.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn test_both_crawlers_fail_with_retry() {
    let mut server = Server::new_async().await;
    let base = server.url();

    // Setup mocks for this scenario: both NPA and RSS fail, should retry after timeout
    let mock_npa_error = mount_npalist_with_error(&mut server).await;
    let mock_rss_error = mount_rss_with_error(&mut server).await;

    // Setup config with short interval to test retry behavior
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
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой)
    );

    // Run for a short time to allow retries
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify both crawlers were called multiple times due to retries
    mock_npa_error.assert_async().await;
    mock_rss_error.assert_async().await;
}

#[tokio::test]
#[serial]
async fn test_multiple_retry_attempts_with_logging() {
    let mut server = Server::new_async().await;
    let base = server.url();

    // Setup mocks for this scenario: both NPA and RSS fail, should retry multiple times
    let mock_npa_error = mount_npalist_with_error(&mut server).await;
    let mock_rss_error = mount_rss_with_error(&mut server).await;

    // Setup config with max_retry_attempts=3 to test multiple retries
    let tf = tempfile::NamedTempFile::new().unwrap();
    let cache = tempfile::tempdir().unwrap();
    
    let cfg_file = render_config_with_retry_limit(
        &base,
        tf.path().to_str().unwrap(),
        cache.path().to_str().unwrap(),
        false, // mastodon_enabled
        false, // telegram_enabled
        true,  // console_enabled
        false, // file_enabled
        true,  // rss_enabled
        true,  // npalist_enabled (будет падать с ошибкой)
        2,     // max_retry_attempts
    );

    // Run for a short time to allow retries
    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify both crawlers were called multiple times due to retries
    mock_npa_error.assert_async().await;
    mock_rss_error.assert_async().await;
}
