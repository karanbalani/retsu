-- Queue configuration is kept in PostgreSQL so every API and worker process
-- resolves the same delivery, retry, and retention rules.
CREATE TABLE queue (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    visibility_timeout_seconds INTEGER NOT NULL,
    max_delivery_attempts SMALLINT NOT NULL,
    default_message_ttl_seconds INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Names are cache keys and metric labels, so bound their encoded size rather
-- than relying on a character count that can hide multi-byte values.
ALTER TABLE queue
ADD CONSTRAINT queue_name_length
CHECK (octet_length(name) BETWEEN 1 AND 64);

-- The API accepts the same portable lowercase identifier format. Enforcing it
-- here protects direct SQL writers and future entrypoints as well.
ALTER TABLE queue
ADD CONSTRAINT queue_name_format
CHECK (name ~ '^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$');

-- A lease may last from one second through six hours.
ALTER TABLE queue
ADD CONSTRAINT queue_visibility_timeout_range
CHECK (visibility_timeout_seconds BETWEEN 1 AND 21600);

-- Bound retry work and keep the value representable by the message counter.
ALTER TABLE queue
ADD CONSTRAINT queue_max_delivery_attempts_range
CHECK (max_delivery_attempts BETWEEN 1 AND 100);

-- Active messages may be retained for at most 30 days.
ALTER TABLE queue
ADD CONSTRAINT queue_default_message_ttl_range
CHECK (default_message_ttl_seconds BETWEEN 1 AND 2592000);

-- Protect audit ordering when configuration is changed by direct SQL.
ALTER TABLE queue
ADD CONSTRAINT queue_updated_at_not_before_created_at
CHECK (updated_at >= created_at);
