CREATE TABLE queue (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    visibility_timeout_seconds INTEGER NOT NULL,
    max_delivery_attempts SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE queue
ADD CONSTRAINT queue_name_length
CHECK (octet_length(name) BETWEEN 1 AND 64);

ALTER TABLE queue
ADD CONSTRAINT queue_name_format
CHECK (name ~ '^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$'); -- alphanumeric, with optional underscores and hyphens

ALTER TABLE queue
ADD CONSTRAINT queue_visibility_timeout_range
CHECK (visibility_timeout_seconds BETWEEN 1 AND 21600); -- 6 hours

ALTER TABLE queue
ADD CONSTRAINT queue_max_delivery_attempts_range
CHECK (max_delivery_attempts BETWEEN 1 AND 100);

ALTER TABLE queue
ADD CONSTRAINT queue_updated_at_not_before_created_at
CHECK (updated_at >= created_at);
