use mastodon_async::scopes::Scopes;
use reqwest::Client;
use std::fs;
use std::path::Path;

use mastodon_async::Language;
use mastodon_async::Registration;
use mastodon_async::data::Data;
use mastodon_async::helpers::cli as m_cli;
// do not touch manifest for secrets
use tracing::{error, info};
use bon::Builder;
use async_trait::async_trait;
use crate::traits::publisher::Publisher;

#[derive(Builder)]
pub struct MastodonPublisher {
    pub client: Client,
    pub base_url: String,
    pub access_token: String,
    pub visibility: Option<String>,
    pub language: Option<String>,
    pub spoiler_text: Option<String>,
    #[builder(default = false)]
    pub sensitive: bool,
    pub max_chars: Option<usize>,
}

impl MastodonPublisher {

    pub async fn post_status(
        &self,
        status: &str,
        visibility: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/v1/statuses", self.base_url.trim_end_matches('/'));
        info!(url = %url, text_len = status.len(), visibility = ?visibility, "mastodon: post_status");
        let mut body = vec![("status", status.to_string())];
        if let Some(v) = visibility {
            body.push(("visibility", v.to_string()));
        }
        let res = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .form(&body)
            .send()
            .await?;
        let code = res.status();
        let text = res.text().await.unwrap_or_default();
        if code.is_success() {
            info!(status = %code, body = %text, "mastodon: post_status ok");
            Ok(())
        } else {
            error!(status = %code, body = %text, "mastodon: post_status error");
            Err(format!("Mastodon error: {}", code).into())
        }
    }

    pub async fn post_status_advanced(
        &self,
        status: &str,
        visibility: Option<&str>,
        language: Option<Language>,
        spoiler_text: Option<&str>,
        sensitive: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/api/v1/statuses", self.base_url.trim_end_matches('/'));
        let mut body: Vec<(&str, String)> = vec![("status", status.to_string())];
        if let Some(v) = visibility {
            body.push(("visibility", v.to_string()));
        }
        if let Some(lang) = language {
            if let Some(code) = lang.to_639_1() {
                body.push(("language", code.to_string()));
            }
        }
        if let Some(sp) = spoiler_text {
            if !sp.is_empty() {
                body.push(("spoiler_text", sp.to_string()));
            }
        }
        if sensitive {
            body.push(("sensitive", "true".to_string()));
        }
        info!(url = %url, text_len = status.len(), visibility = ?visibility, language = ?language, spoiler = ?spoiler_text, sensitive = sensitive, "mastodon: post_status_advanced");
        let res = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .form(&body)
            .send()
            .await?;
        let code = res.status();
        let text = res.text().await.unwrap_or_default();
        if code.is_success() {
            info!(status = %code, body = %text, "mastodon: post_status_advanced ok");
            Ok(())
        } else {
            error!(status = %code, body = %text, "mastodon: post_status_advanced error");
            Err(format!("Mastodon error: {}", code).into())
        }
    }
}

#[async_trait]
impl Publisher for MastodonPublisher {
    fn name(&self) -> &str { "mastodon" }
    async fn publish(&self, _title: &str, _url: &str, text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cut = if let Some(maxc) = self.max_chars { 
            super::utils::trim_with_ellipsis(text, maxc) 
        } else { 
            text.to_string() 
        };
        let lang = self.language.as_deref().unwrap_or("ru");
        let lang = Language::from_639_1(lang);
        let vis = self.visibility.as_deref();
        let spoiler = self.spoiler_text.as_deref().filter(|s| !s.is_empty());
        info!(
            text_len = cut.len(), visibility = ?vis, language = ?self.language, spoiler = ?spoiler,
            sensitive = self.sensitive, "mastodon: publish start"
        );
        match self.post_status_advanced(&cut, vis, lang, spoiler, self.sensitive).await {
            Ok(()) => { info!("mastodon: publish success"); Ok(()) }
            Err(e) => { error!(error = %e, "mastodon: publish failed"); Err(e) }
        }
    }
}

/// Optional interactive login using mastodon-async to obtain token and persist it.
pub async fn ensure_mastodon_token(
    base_url: &str,
    token_path: &Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if token_path.exists() {
        let data = fs::read_to_string(token_path)?;
        let data: Data = serde_yaml::from_str(&data)?;
        if !data.token.is_empty() {
            return Ok(data.token.into_owned());
        }
    }

    // Interactive registration & authentication (stdout/stderr prompts)
    let registration = Registration::new(base_url)
        .client_name("luminis")
        .scopes(Scopes::all())
        .build()
        .await?;
    let mastodon = m_cli::authenticate(registration).await?;

    // Persist credentials
    let data = mastodon.data.clone();
    let serialized = serde_yaml::to_string(&data)?;
    if let Some(parent) = token_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(token_path, serialized)?;
    Ok(data.token.into_owned())
}

/// Load token from secrets file if present; does not initiate CLI login.
pub fn load_token_from_secrets(
    token_path: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    if token_path.exists() {
        let data = fs::read_to_string(token_path)?;
        let data: Data = serde_yaml::from_str(&data)?;
        if !data.token.is_empty() {
            return Ok(Some(data.token.into_owned()));
        }
    }
    Ok(None)
}
