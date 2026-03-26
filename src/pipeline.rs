use futures::stream::{self, StreamExt};
use tracing::{error, info};

use crate::config::AppConfig;
use crate::danbooru::DanbooruClient;
use crate::error::{AppError, Result};
use crate::models::{CacheSource, CaptionOutcome, RangeStats};
use crate::openai::OpenAiClient;
use crate::prompt::build_prompt;
use crate::store::PostgresStore;

#[derive(Clone)]
pub struct Pipeline {
    config: AppConfig,
    danbooru: DanbooruClient,
    openai: OpenAiClient,
    store: PostgresStore,
}

impl Pipeline {
    pub async fn bootstrap(config: AppConfig) -> Result<Self> {
        let store = PostgresStore::connect(&config.database_url).await?;
        store.migrate().await?;
        store.ping().await?;

        Ok(Self {
            danbooru: DanbooruClient::new(config.request_timeout_secs)?,
            openai: OpenAiClient::new(&config).await?,
            store,
            config,
        })
    }

    pub async fn caption_post(&self, post_id: i64) -> Result<CaptionOutcome> {
        info!(post_id, "processing post");

        let source_post = self.danbooru.fetch_post(post_id).await?;
        self.store.upsert_source_post(&source_post).await?;

        let prompt = build_prompt(&self.config, &source_post)?;
        let job_id = self
            .store
            .create_job(post_id, &prompt.fingerprint, &self.config)
            .await?;

        if let Some(cached) = self.store.find_cached_caption(&prompt.fingerprint).await? {
            self.store
                .upsert_result_from_cache(&cached, CacheSource::LocalPg)
                .await?;
            self.store.mark_job_success(job_id, true).await?;

            return Ok(CaptionOutcome {
                post_id: cached.post_id,
                fingerprint: cached.fingerprint,
                cache_source: CacheSource::LocalPg.as_str().to_owned(),
                model: cached.model,
                language: cached.language,
                image_url: cached.image_url,
                usage: cached.usage,
                caption: cached.caption,
            });
        }

        let generation = match self
            .openai
            .generate_caption(&self.config, &source_post, &prompt)
            .await
        {
            Ok(generation) => generation,
            Err(err) => {
                self.best_effort_mark_failure(job_id, &err);
                return Err(err);
            }
        };

        if let Err(err) = self
            .store
            .save_generation(&source_post, &prompt, &self.config, &generation)
            .await
        {
            self.best_effort_mark_failure(job_id, &err);
            return Err(err);
        }

        self.store.mark_job_success(job_id, false).await?;

        Ok(CaptionOutcome {
            post_id: source_post.post_id,
            fingerprint: prompt.fingerprint,
            cache_source: CacheSource::OpenAiFresh.as_str().to_owned(),
            model: self.config.model.clone(),
            language: self.config.language.clone(),
            image_url: source_post.image_url,
            usage: generation.usage,
            caption: generation.caption,
        })
    }

    pub async fn caption_range(
        &self,
        start_id: i64,
        end_id: i64,
        concurrency: usize,
    ) -> Result<RangeStats> {
        if end_id < start_id {
            return Err(AppError::Config(
                "end_id must be greater than or equal to start_id".to_owned(),
            ));
        }

        let mut stats = RangeStats::new(start_id, end_id);
        let results = stream::iter(start_id..=end_id)
            .map(|post_id| {
                let pipeline = self.clone();
                async move { pipeline.caption_post(post_id).await }
            })
            .buffer_unordered(concurrency.max(1))
            .collect::<Vec<_>>()
            .await;

        for result in results {
            match result {
                Ok(outcome) => {
                    if outcome.cache_source == CacheSource::LocalPg.as_str() {
                        stats.record_success(CacheSource::LocalPg);
                    } else {
                        stats.record_success(CacheSource::OpenAiFresh);
                    }
                }
                Err(err) => {
                    error!(error = %err, "range item failed");
                    stats.record_failure();
                }
            }
        }

        Ok(stats)
    }

    fn best_effort_mark_failure(&self, job_id: i64, err: &AppError) {
        let store = self.store.clone();
        let message = err.to_string();

        tokio::spawn(async move {
            if let Err(mark_err) = store.mark_job_failure(job_id, &message).await {
                error!(job_id, error = %mark_err, "failed to persist failed job");
            }
        });
    }
}
