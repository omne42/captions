#![forbid(unsafe_code)]

mod config;
mod danbooru;
mod error;
mod models;
mod openai;
mod pipeline;
mod prompt;
mod store;

use std::sync::Once;

use clap::Parser;

pub use error::{AppError, Result};

use config::{Cli, Command};
use pipeline::Pipeline;

pub async fn run() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = cli.app_config()?;

    match cli.command {
        Command::Migrate => {
            let store = store::PostgresStore::connect(&config.database_url).await?;
            store.migrate().await?;
            store.ping().await?;
            println!("migrations are up to date");
        }
        Command::Post { post_id } => {
            let pipeline = Pipeline::bootstrap(config).await?;
            let outcome = pipeline.caption_post(post_id).await?;
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
        Command::Range {
            start_id,
            end_id,
            concurrency,
        } => {
            let pipeline = Pipeline::bootstrap(config).await?;
            let stats = pipeline
                .caption_range(start_id, end_id, concurrency)
                .await?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
    }

    Ok(())
}

fn init_tracing() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .compact()
            .init();
    });
}
