use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("config load error: {0}")]
    ConfigLoad(#[from] config_kit::Error),
    #[error("ditto l0 error: {0}")]
    Ditto(#[from] ditto_core::error::DittoError),
    #[error("http error: {0}")]
    Http(#[from] http_kit::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("postgres error: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config: {0}")]
    Config(String),
    #[error("source post {0} not found")]
    SourceNotFound(i64),
    #[error("source post {post_id} has no usable image url")]
    MissingImageUrl { post_id: i64 },
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error("invalid model output: {0}")]
    InvalidModelOutput(String),
}
