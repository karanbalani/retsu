-- The DLQ is designed as an append-only set of failure records. Moving an
-- exhausted message here removes it from live-queue claim, TTL, and
-- state-metric paths while preserving its payload and delivery history.
CREATE TABLE queue_dead_letter_message (
    id UUID PRIMARY KEY,
    queue_id UUID NOT NULL REFERENCES queue(id) ON DELETE RESTRICT,
    payload BYTEA NOT NULL,
    priority SMALLINT NOT NULL,
    enqueued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    delivery_attempts SMALLINT NOT NULL,
    last_delivered_at TIMESTAMPTZ NOT NULL,
    dead_lettered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reason TEXT NOT NULL
);

-- Mirrors the live-message priority domain for consistent reporting.
ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_priority_range
CHECK (priority BETWEEN 1 AND 3);

-- Protects historical records from invalid application or maintenance writes.
ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_delivery_attempts_non_negative
CHECK (delivery_attempts >= 0);

-- A bounded reason vocabulary keeps dashboards and alerts low-cardinality.
ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_reason_value
CHECK (reason IN ('MAX_DELIVERY_ATTEMPTS_EXHAUSTED'));

-- Ensures the archived lifecycle reads enqueue -> delivery -> dead-letter,
-- while retaining the original live-message expiry timestamp.
ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_timestamps_consistent
CHECK (
    last_delivered_at >= enqueued_at
    AND expires_at > enqueued_at
    AND dead_lettered_at >= last_delivered_at
);

-- Supports deterministic, queue-scoped history pagination. PostgreSQL can scan
-- the same B-tree backward for newest-first reporting; `id` breaks ties because
-- a batch moved in one transaction can share `dead_lettered_at`.
CREATE INDEX idx_queue_dead_letter_message_queue_id_dead_lettered_at_id
ON queue_dead_letter_message (
    queue_id,
    dead_lettered_at ASC,
    id ASC
);
