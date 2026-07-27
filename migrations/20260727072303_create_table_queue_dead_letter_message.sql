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

ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_priority_range
CHECK (priority BETWEEN 1 AND 3);

ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_delivery_attempts_non_negative
CHECK (delivery_attempts >= 0);

ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_reason_value
CHECK (reason IN ('MAX_DELIVERY_ATTEMPTS_EXHAUSTED'));

ALTER TABLE queue_dead_letter_message
ADD CONSTRAINT queue_dead_letter_message_timestamps_consistent
CHECK (
    last_delivered_at >= enqueued_at
    AND expires_at > enqueued_at
    AND dead_lettered_at >= last_delivered_at
);

CREATE INDEX idx_queue_dead_letter_message_queue_id_dead_lettered_at_id
ON queue_dead_letter_message (
    queue_id,
    dead_lettered_at ASC,
    id ASC
);
