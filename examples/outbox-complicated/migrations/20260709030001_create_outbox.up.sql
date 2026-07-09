CREATE TABLE outbox (
    id          TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    event_type  TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    oss_key     TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
