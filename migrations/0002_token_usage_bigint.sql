ALTER TABLE caption_cache
    ALTER COLUMN input_tokens TYPE BIGINT,
    ALTER COLUMN cached_tokens TYPE BIGINT,
    ALTER COLUMN output_tokens TYPE BIGINT,
    ALTER COLUMN total_tokens TYPE BIGINT;
