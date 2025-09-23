use luminis::run_with_config_path;
use serial_test::serial;
use mockito::Server;
use std::fs;

mod common;

use crate::common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist, mount_stages,
    mount_telegram, prepopulate_cache, read_mocks, render_config,
};

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    let mock_npalist = mount_npalist(&mut server).await;
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
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_only_file_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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

    // Verify mocks were called (cache: stages/gemini not necessarily called)
    mock_npalist.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_console_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    let mock_npalist = mount_npalist(&mut server).await;
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
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    let mock_npalist = mount_npalist(&mut server).await;
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

    // Verify mocks were called (no cache)
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_console_and_file_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_telegram_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram + Mastodon
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
    mock_telegram.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    let mock_npalist = mount_npalist(&mut server).await;
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
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

// Дополнительные тесты для полного покрытия NPAList сценариев

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    let mock_npalist = mount_npalist(&mut server).await;
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

    // Verify mocks were called (no cache)
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_file_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
    mock_telegram.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_console_from_npalist_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    let mock_npalist = mount_npalist(&mut server).await;
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
        true,  // console_enabled
        false, // file_enabled
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (no cache)
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_console_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_mastodon_and_file_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Mastodon
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn publish_telegram_and_console_from_npalist_with_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (_rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: NPAList + Stages + DOCX + Gemini + Telegram
    let mock_npalist = mount_npalist(&mut server).await;
    let _mock_stages = mount_stages(&mut server, &stages_json).await;
    let _mock_docx = mount_docx(&mut server).await;
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
        false, // rss_enabled
        true,  // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();

    // Verify mocks were called (stages/gemini skipped due to cache)
    mock_npalist.assert_async().await;
    mock_telegram.assert_async().await;
}
