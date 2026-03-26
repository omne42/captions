use omne_integrity_primitives::hash_sha256;
use serde_json::json;

use crate::config::AppConfig;
use crate::error::Result;
use crate::models::{PromptBundle, SourcePost};

const PROMPT_VERSION: &str = "2026-03-25";
const SCHEMA_VERSION: &str = "2026-03-25";

const SYSTEM_INSTRUCTIONS: &str = r#"You generate faithful image captions for Danbooru-style artwork.

Rules:
- Inspect the image first. Metadata is only a hint.
- If metadata conflicts with the image, trust the image.
- Keep the summary and short_summary in the requested output language.
- Keep entity arrays high-confidence only. Do not guess names you cannot support visually.
- Do not mention Danbooru, hidden metadata, APIs, or JSON in the caption text.
- Be explicit when the image contains nudity, violence, gore, or fetish content.
- Keep style_summary focused on visual style, rendering, framing, and mood.
- short_summary must be one sentence or two short sentences.
- summary should read like a clean caption a human curator would actually keep.
"#;

pub fn build_prompt(config: &AppConfig, post: &SourcePost) -> Result<PromptBundle> {
    let schema = caption_schema();
    let user_text = build_user_text(config, post);
    let prompt_cache_key = format!(
        "{}:{}:{}:{}:{}",
        config.prompt_cache_namespace,
        config.model,
        config.language,
        PROMPT_VERSION,
        SCHEMA_VERSION
    );
    let fingerprint = compute_fingerprint(json!({
        "model": config.model,
        "language": config.language,
        "prompt_version": PROMPT_VERSION,
        "schema_version": SCHEMA_VERSION,
        "runtime": "ditto_core_l0_openai_responses",
        "image_url": post.image_url,
        "user_text": user_text,
        "schema": schema,
    }))?;

    Ok(PromptBundle {
        instructions: SYSTEM_INSTRUCTIONS.to_owned(),
        user_text,
        schema,
        prompt_cache_key,
        fingerprint,
    })
}

fn build_user_text(config: &AppConfig, post: &SourcePost) -> String {
    format!(
        "Target language: {}\nPost id: {}\nRating hint: {}\nArtist tags: {}\nCharacter tags: {}\nSeries tags: {}\nGeneral tags: {}\nSource url: {}\n\nTask:\n1. Produce a faithful caption from the attached image.\n2. Use metadata as weak hints only.\n3. Keep the output compact and production-ready.\n4. If content warnings are needed, keep them short and literal.",
        config.language,
        post.post_id,
        display_optional(post.rating.as_deref()),
        display_list(&post.artist_tags),
        display_list(&post.character_tags),
        display_list(&post.copyright_tags),
        display_list(&post.general_tags),
        display_optional(post.source_url.as_deref()),
    )
}

fn caption_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "summary",
            "short_summary",
            "style_summary",
            "characters",
            "copyrights",
            "artists",
            "content_warnings"
        ],
        "properties": {
            "summary": {
                "type": "string",
                "description": "A faithful long-form caption in the requested output language."
            },
            "short_summary": {
                "type": "string",
                "description": "A short one-line caption in the requested output language."
            },
            "style_summary": {
                "type": "string",
                "description": "A concise note about rendering style, framing, and mood."
            },
            "characters": {
                "type": "array",
                "items": { "type": "string" }
            },
            "copyrights": {
                "type": "array",
                "items": { "type": "string" }
            },
            "artists": {
                "type": "array",
                "items": { "type": "string" }
            },
            "content_warnings": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_owned()
    } else {
        values.join(", ")
    }
}

fn display_optional(value: Option<&str>) -> String {
    value.unwrap_or("(none)").to_owned()
}

fn compute_fingerprint(value: serde_json::Value) -> Result<String> {
    let bytes = serde_json::to_vec(&value)?;
    Ok(hash_sha256(&bytes).to_string())
}
