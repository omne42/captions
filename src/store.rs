use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;

use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::models::{
    CacheSource, CachedCaption, CaptionPayload, OpenAiGeneration, PromptBundle, SourcePost,
    TokenUsage,
};

struct ResultRecord<'a> {
    post_id: i64,
    fingerprint: &'a str,
    model: &'a str,
    language: &'a str,
    image_url: &'a str,
    caption: &'a CaptionPayload,
    cache_source: CacheSource,
}

#[derive(Clone, Debug)]
pub struct PostgresStore {
    pool: sqlx::PgPool,
}

impl PostgresStore {
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(8).connect(url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!().run(&self.pool).await?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn upsert_source_post(&self, post: &SourcePost) -> Result<()> {
        sqlx::query(
            "INSERT INTO source_posts (
                post_id,
                image_url,
                rating,
                source_url,
                artist_tags,
                character_tags,
                copyright_tags,
                general_tags,
                post_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (post_id) DO UPDATE SET
                image_url = EXCLUDED.image_url,
                rating = EXCLUDED.rating,
                source_url = EXCLUDED.source_url,
                artist_tags = EXCLUDED.artist_tags,
                character_tags = EXCLUDED.character_tags,
                copyright_tags = EXCLUDED.copyright_tags,
                general_tags = EXCLUDED.general_tags,
                post_json = EXCLUDED.post_json,
                fetched_at = NOW()",
        )
        .bind(post.post_id)
        .bind(&post.image_url)
        .bind(&post.rating)
        .bind(&post.source_url)
        .bind(&post.artist_tags)
        .bind(&post.character_tags)
        .bind(&post.copyright_tags)
        .bind(&post.general_tags)
        .bind(Json(post.raw_json.clone()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_job(
        &self,
        post_id: i64,
        fingerprint: &str,
        config: &AppConfig,
    ) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO caption_jobs (post_id, fingerprint, model, language, status)
             VALUES ($1, $2, $3, $4, 'running')
             RETURNING job_id",
        )
        .bind(post_id)
        .bind(fingerprint)
        .bind(&config.model)
        .bind(&config.language)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get("job_id")?)
    }

    pub async fn mark_job_success(&self, job_id: i64, cache_hit: bool) -> Result<()> {
        sqlx::query(
            "UPDATE caption_jobs
             SET status = 'completed', cache_hit = $2, finished_at = NOW(), error_message = NULL
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(cache_hit)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_job_failure(&self, job_id: i64, error_message: &str) -> Result<()> {
        sqlx::query(
            "UPDATE caption_jobs
             SET status = 'failed', error_message = $2, finished_at = NOW()
             WHERE job_id = $1",
        )
        .bind(job_id)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_cached_caption(&self, fingerprint: &str) -> Result<Option<CachedCaption>> {
        let maybe_row = sqlx::query(
            "SELECT
                c.post_id,
                c.fingerprint,
                c.model,
                c.language,
                s.image_url,
                c.caption_json,
                c.input_tokens,
                c.cached_tokens,
                c.output_tokens,
                c.total_tokens
             FROM caption_cache c
             INNER JOIN source_posts s ON s.post_id = c.post_id
             WHERE c.fingerprint = $1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let Json(caption_json): Json<serde_json::Value> = row.try_get("caption_json")?;
        let caption = serde_json::from_value::<CaptionPayload>(caption_json)?.validate()?;

        Ok(Some(CachedCaption {
            post_id: row.try_get("post_id")?,
            fingerprint: row.try_get("fingerprint")?,
            model: row.try_get("model")?,
            language: row.try_get("language")?,
            image_url: row.try_get("image_url")?,
            caption,
            usage: TokenUsage {
                input_tokens: decode_token_usage(row.try_get("input_tokens")?, "input_tokens")?,
                cached_tokens: decode_token_usage(row.try_get("cached_tokens")?, "cached_tokens")?,
                output_tokens: decode_token_usage(row.try_get("output_tokens")?, "output_tokens")?,
                total_tokens: decode_token_usage(row.try_get("total_tokens")?, "total_tokens")?,
            },
        }))
    }

    pub async fn upsert_result_from_cache(
        &self,
        cached: &CachedCaption,
        cache_source: CacheSource,
    ) -> Result<()> {
        self.upsert_result_row(&ResultRecord {
            post_id: cached.post_id,
            fingerprint: &cached.fingerprint,
            model: &cached.model,
            language: &cached.language,
            image_url: &cached.image_url,
            caption: &cached.caption,
            cache_source,
        })
        .await
    }

    pub async fn save_generation(
        &self,
        post: &SourcePost,
        prompt: &PromptBundle,
        config: &AppConfig,
        generation: &OpenAiGeneration,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO caption_cache (
                fingerprint,
                post_id,
                model,
                language,
                prompt_version,
                schema_version,
                prompt_cache_key,
                request_json,
                response_json,
                caption_json,
                input_tokens,
                cached_tokens,
                output_tokens,
                total_tokens,
                openai_response_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (fingerprint) DO UPDATE SET
                post_id = EXCLUDED.post_id,
                model = EXCLUDED.model,
                language = EXCLUDED.language,
                prompt_cache_key = EXCLUDED.prompt_cache_key,
                request_json = EXCLUDED.request_json,
                response_json = EXCLUDED.response_json,
                caption_json = EXCLUDED.caption_json,
                input_tokens = EXCLUDED.input_tokens,
                cached_tokens = EXCLUDED.cached_tokens,
                output_tokens = EXCLUDED.output_tokens,
                total_tokens = EXCLUDED.total_tokens,
                openai_response_id = EXCLUDED.openai_response_id",
        )
        .bind(&prompt.fingerprint)
        .bind(post.post_id)
        .bind(&config.model)
        .bind(&config.language)
        .bind("2026-03-25")
        .bind("2026-03-25")
        .bind(&prompt.prompt_cache_key)
        .bind(Json(generation.request_json.clone()))
        .bind(Json(generation.response_json.clone()))
        .bind(Json(serde_json::to_value(&generation.caption)?))
        .bind(encode_token_usage(
            generation.usage.input_tokens,
            "input_tokens",
        )?)
        .bind(encode_token_usage(
            generation.usage.cached_tokens,
            "cached_tokens",
        )?)
        .bind(encode_token_usage(
            generation.usage.output_tokens,
            "output_tokens",
        )?)
        .bind(encode_token_usage(
            generation.usage.total_tokens,
            "total_tokens",
        )?)
        .bind(generation.response_id.as_deref())
        .execute(&mut *tx)
        .await?;

        self.upsert_result_row_with_executor(
            &mut *tx,
            &ResultRecord {
                post_id: post.post_id,
                fingerprint: &prompt.fingerprint,
                model: &config.model,
                language: &config.language,
                image_url: &post.image_url,
                caption: &generation.caption,
                cache_source: CacheSource::OpenAiFresh,
            },
        )
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn upsert_result_row(&self, record: &ResultRecord<'_>) -> Result<()> {
        self.upsert_result_row_with_executor(&self.pool, record)
            .await
    }

    async fn upsert_result_row_with_executor<'e, E>(
        &self,
        executor: E,
        record: &ResultRecord<'_>,
    ) -> Result<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            "INSERT INTO caption_results (
                post_id,
                fingerprint,
                model,
                language,
                image_url,
                summary,
                short_summary,
                caption_json,
                cache_source
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (post_id) DO UPDATE SET
                fingerprint = EXCLUDED.fingerprint,
                model = EXCLUDED.model,
                language = EXCLUDED.language,
                image_url = EXCLUDED.image_url,
                summary = EXCLUDED.summary,
                short_summary = EXCLUDED.short_summary,
                caption_json = EXCLUDED.caption_json,
                cache_source = EXCLUDED.cache_source,
                updated_at = NOW()",
        )
        .bind(record.post_id)
        .bind(record.fingerprint)
        .bind(record.model)
        .bind(record.language)
        .bind(record.image_url)
        .bind(&record.caption.summary)
        .bind(&record.caption.short_summary)
        .bind(Json(serde_json::to_value(record.caption)?))
        .bind(record.cache_source.as_str())
        .execute(executor)
        .await?;

        Ok(())
    }
}

fn encode_token_usage(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| {
        AppError::InvalidTokenUsage(format!("{field} exceeds PostgreSQL BIGINT range: {value}"))
    })
}

fn decode_token_usage(value: i64, field: &str) -> Result<u64> {
    u64::try_from(value)
        .map_err(|_| AppError::InvalidTokenUsage(format!("{field} must not be negative: {value}")))
}

#[cfg(test)]
mod tests {
    use super::{decode_token_usage, encode_token_usage};

    #[test]
    fn encode_token_usage_rejects_values_above_i64_max() {
        let err = encode_token_usage(i64::MAX as u64 + 1, "total_tokens")
            .expect_err("value above bigint should fail");
        assert!(err.to_string().contains("BIGINT"));
    }

    #[test]
    fn decode_token_usage_rejects_negative_database_values() {
        let err = decode_token_usage(-1, "total_tokens").expect_err("negative value must fail");
        assert!(err.to_string().contains("must not be negative"));
    }
}
