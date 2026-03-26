use std::env;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use config_kit::{ConfigLoadOptions, SchemaConfigLoader, SchemaFileLayerOptions};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4.1";
const DEFAULT_LANGUAGE: &str = "zh";
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1200;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;
const DEFAULT_PROMPT_CACHE_NAMESPACE: &str = "captions";
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const DEFAULT_CONFIG_CANDIDATES: [&str; 6] = [
    "captions.toml",
    "captions.yaml",
    "captions.json",
    ".captions/config.toml",
    ".captions/config.yaml",
    ".captions/config.json",
];

#[derive(Debug, Parser)]
#[command(
    name = "captions",
    version,
    about = "Danbooru caption pipeline built on OpenAI Responses"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[arg(long, global = true, env = "CAPTIONS_DATABASE_URL")]
    database_url: Option<String>,

    #[arg(long, global = true, env = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,

    #[arg(long, global = true, env = "CAPTIONS_OPENAI_BASE_URL")]
    openai_base_url: Option<String>,

    #[arg(long, global = true, env = "CAPTIONS_MODEL")]
    model: Option<String>,

    #[arg(long, global = true, env = "CAPTIONS_LANGUAGE")]
    language: Option<String>,

    #[arg(long, global = true, env = "CAPTIONS_MAX_OUTPUT_TOKENS")]
    max_output_tokens: Option<u32>,

    #[arg(long, global = true, env = "CAPTIONS_REQUEST_TIMEOUT_SECS")]
    request_timeout_secs: Option<u64>,

    #[arg(long, global = true, env = "CAPTIONS_PROMPT_CACHE_NAMESPACE")]
    prompt_cache_namespace: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Migrate,
    Post {
        #[arg(long)]
        post_id: i64,
    },
    Range {
        #[arg(long)]
        start_id: i64,
        #[arg(long)]
        end_id: i64,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: String,
    pub openai_api_key: Option<String>,
    pub openai_base_url: String,
    pub model: String,
    pub language: String,
    pub max_output_tokens: u32,
    pub request_timeout_secs: u64,
    pub prompt_cache_namespace: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AppConfigLayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    database_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    openai_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    openai_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_namespace: Option<String>,
}

impl Cli {
    pub fn app_config(&self) -> Result<AppConfig> {
        let root = env::current_dir()?;
        self.app_config_in_root(root.as_path())
    }

    fn app_config_in_root(&self, root: &Path) -> Result<AppConfig> {
        let cli_layer = self.config_layer();
        let merged =
            load_config_layer_with_env_lookup(root, self.config.as_deref(), &cli_layer, |name| {
                env::var(name).ok()
            })?;
        AppConfig::from_layer(merged)
    }

    fn config_layer(&self) -> AppConfigLayer {
        AppConfigLayer {
            database_url: self
                .database_url
                .clone()
                .or_else(|| env::var("DATABASE_URL").ok()),
            openai_api_key: self.openai_api_key.clone(),
            openai_base_url: self.openai_base_url.clone(),
            model: self.model.clone(),
            language: self.language.clone(),
            max_output_tokens: self.max_output_tokens,
            request_timeout_secs: self.request_timeout_secs,
            prompt_cache_namespace: self.prompt_cache_namespace.clone(),
        }
    }
}

impl AppConfig {
    fn from_layer(layer: AppConfigLayer) -> Result<Self> {
        let database_url = resolve_required_string(
            layer.database_url,
            "missing captions database url (set CAPTIONS_DATABASE_URL, DATABASE_URL, --database-url, or config file database_url)",
        )?;
        let max_output_tokens =
            resolve_u32_with_default(layer.max_output_tokens, DEFAULT_MAX_OUTPUT_TOKENS);
        if max_output_tokens == 0 {
            return Err(AppError::Config(
                "max_output_tokens must be greater than zero".to_owned(),
            ));
        }

        let request_timeout_secs =
            resolve_u64_with_default(layer.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        if request_timeout_secs == 0 {
            return Err(AppError::Config(
                "request_timeout_secs must be greater than zero".to_owned(),
            ));
        }

        Ok(Self {
            database_url,
            openai_api_key: normalize_optional_string(layer.openai_api_key),
            openai_base_url: resolve_string_with_default(
                layer.openai_base_url,
                DEFAULT_OPENAI_BASE_URL,
            )
            .trim_end_matches('/')
            .to_owned(),
            model: resolve_string_with_default(layer.model, DEFAULT_MODEL),
            language: resolve_string_with_default(layer.language, DEFAULT_LANGUAGE),
            max_output_tokens,
            request_timeout_secs,
            prompt_cache_namespace: resolve_string_with_default(
                layer.prompt_cache_namespace,
                DEFAULT_PROMPT_CACHE_NAMESPACE,
            ),
        })
    }
}

fn load_config_layer_with_env_lookup<F>(
    root: &Path,
    explicit_config: Option<&Path>,
    cli_layer: &AppConfigLayer,
    mut lookup: F,
) -> Result<AppConfigLayer>
where
    F: FnMut(&str) -> Option<String>,
{
    let loader = match explicit_config {
        Some(path) => SchemaConfigLoader::new().add_file_layer(
            "captions config",
            resolve_config_path(root, path),
            schema_file_layer_options(true),
        ),
        None => SchemaConfigLoader::new().add_candidate_file_layer(
            "captions config",
            root,
            DEFAULT_CONFIG_CANDIDATES,
            schema_file_layer_options(false),
        ),
    };

    Ok(loader
        .add_serializable_layer("cli/env", cli_layer)?
        .load_with_env_lookup::<AppConfigLayer, _>(|name| lookup(name))?
        .into_value())
}

fn resolve_config_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn schema_file_layer_options(required: bool) -> SchemaFileLayerOptions {
    SchemaFileLayerOptions::new()
        .required(required)
        .with_load_options(ConfigLoadOptions::new().with_max_bytes(MAX_CONFIG_BYTES))
        .with_env_interpolation(true)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn resolve_required_string(value: Option<String>, missing_message: &str) -> Result<String> {
    normalize_optional_string(value).ok_or_else(|| AppError::Config(missing_message.to_owned()))
}

fn resolve_string_with_default(value: Option<String>, default: &str) -> String {
    normalize_optional_string(value).unwrap_or_else(|| default.to_owned())
}

fn resolve_u32_with_default(value: Option<u32>, default: u32) -> u32 {
    value.unwrap_or(default)
}

fn resolve_u64_with_default(value: Option<u64>, default: u64) -> u64 {
    value.unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cli() -> Cli {
        Cli {
            command: Command::Migrate,
            config: None,
            database_url: None,
            openai_api_key: None,
            openai_base_url: None,
            model: None,
            language: None,
            max_output_tokens: None,
            request_timeout_secs: None,
            prompt_cache_namespace: None,
        }
    }

    #[test]
    fn explicit_yaml_config_is_loaded_via_config_kit() {
        let dir = tempfile::tempdir().expect("dir");
        let config_path = dir.path().join("captions.yaml");
        std::fs::write(
            &config_path,
            concat!(
                "database_url: postgres://captions:test@127.0.0.1:5432/captions\n",
                "openai_base_url: https://example.com/v1/\n",
                "model: gpt-4.1-mini\n",
                "language: en\n",
                "max_output_tokens: 256\n",
                "request_timeout_secs: 45\n",
                "prompt_cache_namespace: captions-dev\n",
            ),
        )
        .expect("write config");

        let mut cli = base_cli();
        cli.config = Some(config_path);

        let merged = load_config_layer_with_env_lookup(
            dir.path(),
            cli.config.as_deref(),
            &AppConfigLayer {
                database_url: cli.database_url.clone(),
                openai_api_key: cli.openai_api_key.clone(),
                openai_base_url: cli.openai_base_url.clone(),
                model: cli.model.clone(),
                language: cli.language.clone(),
                max_output_tokens: cli.max_output_tokens,
                request_timeout_secs: cli.request_timeout_secs,
                prompt_cache_namespace: cli.prompt_cache_namespace.clone(),
            },
            |_name| None,
        )
        .expect("load");
        let config = AppConfig::from_layer(merged).expect("config");
        assert_eq!(
            config.database_url,
            "postgres://captions:test@127.0.0.1:5432/captions"
        );
        assert_eq!(config.openai_base_url, "https://example.com/v1");
        assert_eq!(config.model, "gpt-4.1-mini");
        assert_eq!(config.language, "en");
        assert_eq!(config.max_output_tokens, 256);
        assert_eq!(config.request_timeout_secs, 45);
        assert_eq!(config.prompt_cache_namespace, "captions-dev");
    }

    #[test]
    fn discovered_config_is_merged_with_cli_layer() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("captions.toml"),
            concat!(
                "database_url = \"postgres://captions:test@127.0.0.1:5432/captions\"\n",
                "model = \"gpt-4.1-mini\"\n",
                "language = \"en\"\n",
            ),
        )
        .expect("write config");

        let mut cli = base_cli();
        cli.model = Some("gpt-4.1-nano".to_owned());
        cli.request_timeout_secs = Some(99);

        let merged = load_config_layer_with_env_lookup(
            dir.path(),
            None,
            &AppConfigLayer {
                database_url: cli.database_url.clone(),
                openai_api_key: cli.openai_api_key.clone(),
                openai_base_url: cli.openai_base_url.clone(),
                model: cli.model.clone(),
                language: cli.language.clone(),
                max_output_tokens: cli.max_output_tokens,
                request_timeout_secs: cli.request_timeout_secs,
                prompt_cache_namespace: cli.prompt_cache_namespace.clone(),
            },
            |_name| None,
        )
        .expect("load");
        let config = AppConfig::from_layer(merged).expect("config");
        assert_eq!(
            config.database_url,
            "postgres://captions:test@127.0.0.1:5432/captions"
        );
        assert_eq!(config.model, "gpt-4.1-nano");
        assert_eq!(config.language, "en");
        assert_eq!(config.request_timeout_secs, 99);
    }

    #[test]
    fn config_supports_env_interpolation() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("captions.json"),
            r#"{
  "database_url": "${CAPTIONS_DB}",
  "prompt_cache_namespace": "${PROMPT_NS}"
}"#,
        )
        .expect("write config");

        let cli = base_cli();
        let merged =
            load_config_layer_with_env_lookup(dir.path(), None, &cli.config_layer(), |name| {
                match name {
                    "CAPTIONS_DB" => {
                        Some("postgres://captions:test@127.0.0.1:5432/captions".to_owned())
                    }
                    "PROMPT_NS" => Some("captions-ci".to_owned()),
                    _ => None,
                }
            })
            .expect("load");

        let config = AppConfig::from_layer(merged).expect("resolve");
        assert_eq!(
            config.database_url,
            "postgres://captions:test@127.0.0.1:5432/captions"
        );
        assert_eq!(config.prompt_cache_namespace, "captions-ci");
    }

    #[test]
    fn database_url_is_required_after_merge() {
        let cli = base_cli();
        let merged = load_config_layer_with_env_lookup(
            Path::new("."),
            None,
            &AppConfigLayer {
                database_url: cli.database_url.clone(),
                openai_api_key: cli.openai_api_key.clone(),
                openai_base_url: cli.openai_base_url.clone(),
                model: cli.model.clone(),
                language: cli.language.clone(),
                max_output_tokens: cli.max_output_tokens,
                request_timeout_secs: cli.request_timeout_secs,
                prompt_cache_namespace: cli.prompt_cache_namespace.clone(),
            },
            |_name| None,
        )
        .expect("load");
        let err = AppConfig::from_layer(merged).expect_err("missing database url must fail");
        assert!(
            err.to_string().contains("missing captions database url"),
            "{err}"
        );
    }
}
