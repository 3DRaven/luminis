use luminis::run_with_config_path;
use luminis::services::documents::DocxMarkdownFetcher;
use luminis::traits::chat_api::ChatApi;
use luminis::traits::markdown_fetcher::MarkdownFetcher;
use serial_test::serial;
use mockito::Server;
use std::fs;

mod common;

use common::{
    mount_docx, mount_gemini_generate, mount_mastodon, mount_npalist, mount_rss, mount_stages,
    mount_telegram, read_mocks, render_config,
};


#[tokio::test]
#[serial]
async fn publish_all_publishers_from_both_sources_without_cache() {
    let mut server = Server::new_async().await;
    let base = server.url();
    let (rss_xml, stages_json) = read_mocks();

    // Setup mocks for this scenario: RSS + NPAList + Stages + DOCX + Gemini + Telegram + Mastodon
    let _mock_rss = mount_rss(&mut server, &rss_xml).await;
    let mock_npalist = mount_npalist(&mut server).await;
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
        true, // mastodon_enabled
        true, // telegram_enabled
        true, // console_enabled
        true, // file_enabled
        true, // rss_enabled
        true, // npalist_enabled
    );

    let _ = run_with_config_path(cfg_file.path().to_str().unwrap())
        .await
        .unwrap();
    let out = fs::read_to_string(tf.path()).unwrap();
    assert!(
        !out.trim().is_empty(),
        "output file must contain published post"
    );

    // Verify mocks were called (no cache). RSS может не вызываться в этом прогоне.
    mock_npalist.assert_async().await;
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
    mock_gemini.assert_async().await;
    mock_telegram.assert_async().await;
    mock_mastodon.assert_async().await;
}

#[tokio::test]
#[serial]
async fn fetch_docx_via_wiremock() {
    let mut server = Server::new_async().await;
    let base = server.url();

    // Setup mocks using test_utils
    let (_rss_xml, stages_json) = read_mocks();
    let mock_stages = mount_stages(&mut server, &stages_json).await;
    let mock_docx = mount_docx(&mut server).await;
    // Call MarkdownFetcher (Docx implementation) directly
    let fetcher = DocxMarkdownFetcher::builder().file_id_url_template(format!(
        "{}/api/public/PublicProjects/GetProjectStages/{{project_id}}",
        base
    )).build();
    let res = fetcher.fetch_markdown("160532").await.unwrap();
    assert!(res.is_some(), "DOCX should be fetched and parsed");
    let (_bytes, md) = res.unwrap();
    assert!(
        !md.trim().is_empty(),
        "Extracted markdown should not be empty"
    );
    // Verify mocks were called
    mock_stages.assert_async().await;
    mock_docx.assert_async().await;
}

#[tokio::test]
#[serial]
async fn test_gemini_api_client() {
    let mut mock_server = Server::new_async().await;
    let base = format!("{}/v1beta", mock_server.url());

    let prompt_text = "Write a story about a magic backpack.";

    // Setup Gemini mock using test_utils
    mount_gemini_generate(&mut mock_server).await;

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
    assert!(resp.contains("Поправки в закон об ОМС"));
}
