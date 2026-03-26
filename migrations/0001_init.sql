CREATE TABLE IF NOT EXISTS source_posts (
    post_id BIGINT PRIMARY KEY,
    image_url TEXT NOT NULL,
    rating TEXT,
    source_url TEXT,
    artist_tags TEXT[] NOT NULL DEFAULT '{}',
    character_tags TEXT[] NOT NULL DEFAULT '{}',
    copyright_tags TEXT[] NOT NULL DEFAULT '{}',
    general_tags TEXT[] NOT NULL DEFAULT '{}',
    post_json JSONB NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS caption_cache (
    fingerprint TEXT PRIMARY KEY,
    post_id BIGINT NOT NULL REFERENCES source_posts(post_id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    language TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    prompt_cache_key TEXT NOT NULL,
    request_json JSONB NOT NULL,
    response_json JSONB NOT NULL,
    caption_json JSONB NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    openai_response_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS caption_results (
    post_id BIGINT PRIMARY KEY REFERENCES source_posts(post_id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL REFERENCES caption_cache(fingerprint),
    model TEXT NOT NULL,
    language TEXT NOT NULL,
    image_url TEXT NOT NULL,
    summary TEXT NOT NULL,
    short_summary TEXT NOT NULL,
    caption_json JSONB NOT NULL,
    cache_source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS caption_jobs (
    job_id BIGSERIAL PRIMARY KEY,
    post_id BIGINT NOT NULL,
    fingerprint TEXT NOT NULL,
    model TEXT NOT NULL,
    language TEXT NOT NULL,
    status TEXT NOT NULL,
    cache_hit BOOLEAN NOT NULL DEFAULT FALSE,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_caption_jobs_post_id_started_at
    ON caption_jobs (post_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_caption_jobs_status_started_at
    ON caption_jobs (status, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_caption_cache_post_id_created_at
    ON caption_cache (post_id, created_at DESC);
