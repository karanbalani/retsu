CREATE TABLE queue_message (
    id UUID PRIMARY KEY,
    queue_id UUID NOT NULL REFERENCES queue(id) ON DELETE RESTRICT,
    enqueue_order BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    payload BYTEA NOT NULL,
    priority SMALLINT NOT NULL,
    state TEXT NOT NULL DEFAULT 'READY',
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    delivery_attempts SMALLINT NOT NULL DEFAULT 0,
    receipt_handle UUID,
    available_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_delivered_at TIMESTAMPTZ
);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_priority_range
CHECK (priority BETWEEN 1 AND 3);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_state_value
CHECK (state in ('READY', 'IN_FLIGHT'));

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_delivery_attempts_non_negative
CHECK (delivery_attempts >= 0);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_expiration_after_enqueue
CHECK (
    expires_at > enqueued_at
    AND expires_at <= enqueued_at + INTERVAL '30 days'
);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_state_consistency
CHECK (
    (
        state = 'READY'
        AND receipt_handle IS NULL
        AND last_delivered_at IS NULL
        AND delivery_attempts = 0
    )
    OR
    (
        state = 'IN_FLIGHT'
        AND receipt_handle IS NOT NULL
        AND last_delivered_at IS NOT NULL
        AND delivery_attempts > 0
    )
);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_availability_after_enqueue_or_delivery
CHECK (
    (
        last_delivered_at IS NULL
        AND available_after >= enqueued_at
    )
    OR (
        last_delivered_at IS NOT NULL
        AND available_after > last_delivered_at
    )
);

ALTER TABLE queue_message
ADD CONSTRAINT queue_message_delivery_not_before_enqueue
CHECK (
    last_delivered_at IS NULL
    OR last_delivered_at >= enqueued_at
);

CREATE INDEX idx_queue_message_ready_dequeue
ON queue_message (
    queue_id,
    priority DESC,
    enqueue_order ASC
)
WHERE state = 'READY';

CREATE INDEX idx_queue_message_expired_leases
ON queue_message (available_after)
WHERE state = 'IN_FLIGHT';

CREATE INDEX idx_queue_message_expiration
ON queue_message (expires_at);
