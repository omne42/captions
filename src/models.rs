use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, Result};

const DANBOORU_HOST: &str = "https://danbooru.donmai.us";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DanbooruPost {
    pub id: i64,
    pub large_file_url: Option<String>,
    pub file_url: Option<String>,
    pub source: Option<String>,
    pub rating: Option<String>,
    pub tag_string_general: Option<String>,
    pub tag_string_character: Option<String>,
    pub tag_string_artist: Option<String>,
    pub tag_string_copyright: Option<String>,
    pub file_ext: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SourcePost {
    pub post_id: i64,
    pub image_url: String,
    pub rating: Option<String>,
    pub source_url: Option<String>,
    pub artist_tags: Vec<String>,
    pub character_tags: Vec<String>,
    pub copyright_tags: Vec<String>,
    pub general_tags: Vec<String>,
    pub raw_json: Value,
}

impl SourcePost {
    pub fn from_wire(post: DanbooruPost, raw_json: Value) -> Result<Self> {
        let image_url = post
            .large_file_url
            .or(post.file_url)
            .map(|url| normalize_url(&url))
            .ok_or(AppError::MissingImageUrl { post_id: post.id })?;

        Ok(Self {
            post_id: post.id,
            image_url,
            rating: post.rating.filter(|value| !value.trim().is_empty()),
            source_url: post.source.filter(|value| !value.trim().is_empty()),
            artist_tags: split_unique_tags(post.tag_string_artist),
            character_tags: split_unique_tags(post.tag_string_character),
            copyright_tags: split_unique_tags(post.tag_string_copyright),
            general_tags: split_unique_tags(post.tag_string_general),
            raw_json,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionPayload {
    pub summary: String,
    pub short_summary: String,
    pub style_summary: String,
    #[serde(default)]
    pub characters: Vec<String>,
    #[serde(default)]
    pub copyrights: Vec<String>,
    #[serde(default)]
    pub artists: Vec<String>,
    #[serde(default)]
    pub content_warnings: Vec<String>,
}

impl CaptionPayload {
    pub fn validate(mut self) -> Result<Self> {
        self.summary = self.summary.trim().to_owned();
        self.short_summary = self.short_summary.trim().to_owned();
        self.style_summary = self.style_summary.trim().to_owned();

        if self.summary.is_empty() {
            return Err(AppError::InvalidModelOutput(
                "summary must not be empty".to_owned(),
            ));
        }
        if self.short_summary.is_empty() {
            return Err(AppError::InvalidModelOutput(
                "short_summary must not be empty".to_owned(),
            ));
        }
        if self.style_summary.is_empty() {
            return Err(AppError::InvalidModelOutput(
                "style_summary must not be empty".to_owned(),
            ));
        }

        trim_vec(&mut self.characters);
        trim_vec(&mut self.copyrights);
        trim_vec(&mut self.artists);
        trim_vec(&mut self.content_warnings);

        Ok(self)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub cached_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct PromptBundle {
    pub instructions: String,
    pub user_text: String,
    pub schema: Value,
    pub prompt_cache_key: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiGeneration {
    pub response_id: Option<String>,
    pub request_json: Value,
    pub response_json: Value,
    pub caption: CaptionPayload,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct CachedCaption {
    pub post_id: i64,
    pub fingerprint: String,
    pub model: String,
    pub language: String,
    pub image_url: String,
    pub caption: CaptionPayload,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Copy)]
pub enum CacheSource {
    LocalPg,
    OpenAiFresh,
}

impl CacheSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalPg => "local_pg",
            Self::OpenAiFresh => "openai_fresh",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptionOutcome {
    pub post_id: i64,
    pub fingerprint: String,
    pub cache_source: String,
    pub model: String,
    pub language: String,
    pub image_url: String,
    pub usage: TokenUsage,
    pub caption: CaptionPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct RangeStats {
    pub start_id: i64,
    pub end_id: i64,
    pub total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub cache_hits: u64,
    pub openai_calls: u64,
}

impl RangeStats {
    pub fn new(start_id: i64, end_id: i64) -> Self {
        let total = if end_id >= start_id {
            (end_id - start_id + 1) as u64
        } else {
            0
        };

        Self {
            start_id,
            end_id,
            total,
            succeeded: 0,
            failed: 0,
            cache_hits: 0,
            openai_calls: 0,
        }
    }

    pub fn record_success(&mut self, cache_source: CacheSource) {
        self.succeeded += 1;
        match cache_source {
            CacheSource::LocalPg => self.cache_hits += 1,
            CacheSource::OpenAiFresh => self.openai_calls += 1,
        }
    }

    pub fn record_failure(&mut self) {
        self.failed += 1;
    }
}

fn split_unique_tags(value: Option<String>) -> Vec<String> {
    let mut tags = Vec::new();

    for candidate in value.unwrap_or_default().split_whitespace() {
        if !tags.iter().any(|tag| tag == candidate) {
            tags.push(candidate.to_owned());
        }
    }

    tags
}

fn trim_vec(values: &mut Vec<String>) {
    values.retain(|value| !value.trim().is_empty());
    for value in values.iter_mut() {
        *value = value.trim().to_owned();
    }
}

fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_owned()
    } else if url.starts_with('/') {
        format!("{DANBOORU_HOST}{url}")
    } else {
        format!("{DANBOORU_HOST}/{url}")
    }
}
