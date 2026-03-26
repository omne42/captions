use std::sync::Arc;
use std::time::Duration;

use ditto_core::capabilities::text::LanguageModelTextExt;
use ditto_core::config::{Env, ProviderApi, ProviderAuth, ProviderConfig};
use ditto_core::contracts::{ContentPart, GenerateRequest, ImageSource, Message, Role};
use ditto_core::llm_core::model::LanguageModel;
use ditto_core::provider_options::{
    JsonSchemaFormat, ProviderOptions, ResponseFormat, request_with_provider_options,
};
use ditto_core::runtime::build_language_model;
use serde_json::json;
use tokio::time::timeout;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::models::{CaptionPayload, OpenAiGeneration, PromptBundle, SourcePost, TokenUsage};

#[derive(Clone)]
pub struct OpenAiClient {
    llm: Arc<dyn LanguageModel>,
}

impl OpenAiClient {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        let api_key = config
            .openai_api_key
            .clone()
            .ok_or_else(|| AppError::Config("missing OPENAI_API_KEY".to_owned()))?;
        let mut env = Env::default();
        env.dotenv.insert("OPENAI_API_KEY".to_owned(), api_key);
        let provider_config = ProviderConfig {
            provider: Some("openai".to_owned()),
            base_url: Some(config.openai_base_url.clone()),
            default_model: Some(config.model.clone()),
            auth: Some(ProviderAuth::ApiKeyEnv {
                keys: vec!["OPENAI_API_KEY".to_owned()],
            }),
            upstream_api: Some(ProviderApi::OpenaiResponses),
            ..ProviderConfig::default()
        };
        let llm = build_language_model("openai", &provider_config, &env).await?;

        Ok(Self { llm })
    }

    pub async fn generate_caption(
        &self,
        config: &AppConfig,
        post: &SourcePost,
        prompt: &PromptBundle,
    ) -> Result<OpenAiGeneration> {
        let messages = vec![
            Message::system(&prompt.instructions),
            Message {
                role: Role::User,
                content: vec![
                    ContentPart::Text {
                        text: prompt.user_text.clone(),
                    },
                    ContentPart::Image {
                        source: ImageSource::Url {
                            url: post.image_url.clone(),
                        },
                    },
                ],
            },
        ];
        let request = request_with_provider_options(
            GenerateRequest {
                model: Some(config.model.clone()),
                max_tokens: Some(config.max_output_tokens),
                ..GenerateRequest::from(messages)
            },
            ProviderOptions {
                response_format: Some(ResponseFormat::JsonSchema {
                    json_schema: JsonSchemaFormat {
                        name: "caption_result".to_owned(),
                        schema: prompt.schema.clone(),
                        strict: Some(true),
                    },
                }),
                prompt_cache_key: Some(prompt.prompt_cache_key.clone()),
                ..ProviderOptions::default()
            },
        )?;
        let request_json = serde_json::to_value(&request)?;
        let generated = timeout(
            Duration::from_secs(config.request_timeout_secs),
            self.llm.generate_text(request),
        )
        .await
        .map_err(|_| {
            AppError::Upstream(format!(
                "openai responses request timed out after {} seconds",
                config.request_timeout_secs
            ))
        })??;
        let response_json = json!({
            "provider_metadata": generated.response.provider_metadata,
            "finish_reason": generated.response.finish_reason,
            "usage": generated.response.usage,
            "warnings": generated.response.warnings,
            "content": generated.response.content,
        });
        let caption = serde_json::from_str::<CaptionPayload>(&generated.text)?.validate()?;
        let response_id = response_json
            .get("provider_metadata")
            .and_then(|value| value.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);

        Ok(OpenAiGeneration {
            response_id,
            request_json,
            response_json,
            caption,
            usage: TokenUsage {
                input_tokens: generated.response.usage.input_tokens.unwrap_or_default() as u32,
                cached_tokens: generated
                    .response
                    .usage
                    .cache_input_tokens
                    .unwrap_or_default() as u32,
                output_tokens: generated.response.usage.output_tokens.unwrap_or_default() as u32,
                total_tokens: generated.response.usage.total_tokens.unwrap_or_default() as u32,
            },
        })
    }
}
