use std::time::Duration;

use http_kit::{
    HttpClientOptions, build_http_client_with_options, read_json_body_limited,
    read_text_body_limited, send_reqwest,
};
use reqwest::StatusCode;

use crate::error::{AppError, Result};
use crate::models::{DanbooruPost, SourcePost};

#[derive(Clone, Debug)]
pub struct DanbooruClient {
    http: reqwest::Client,
}

impl DanbooruClient {
    pub fn new(timeout_secs: u64) -> Result<Self> {
        let http = build_http_client_with_options(&HttpClientOptions {
            timeout: Some(Duration::from_secs(timeout_secs)),
            connect_timeout: Some(Duration::from_secs(10)),
            follow_redirects: true,
            no_proxy: false,
            default_headers: reqwest::header::HeaderMap::new(),
        })?;

        Ok(Self { http })
    }

    pub async fn fetch_post(&self, post_id: i64) -> Result<SourcePost> {
        let url = format!("https://danbooru.donmai.us/posts/{post_id}.json");
        let response = send_reqwest(self.http.get(&url), "fetch danbooru post").await?;
        let status = response.status();

        if status == StatusCode::NOT_FOUND {
            return Err(AppError::SourceNotFound(post_id));
        }
        if !status.is_success() {
            let body = read_text_body_limited(response, 16 * 1024).await?;
            return Err(AppError::Upstream(format!(
                "danbooru returned {status} for post {post_id}: {body}"
            )));
        }

        let raw_json = read_json_body_limited(response, 256 * 1024).await?;
        let post: DanbooruPost = serde_json::from_value(raw_json.clone())?;
        SourcePost::from_wire(post, raw_json)
    }
}
