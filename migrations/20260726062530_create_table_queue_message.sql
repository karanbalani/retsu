-- `state` records the last explicit lifecycle transition, while
-- `available_after` supplies the time-based visibility boundary. An IN_FLIGHT
-- row whose lease has elapsed remains physically IN_FLIGHT but is logically
-- ready for retry; no background UPDATE is needed to requeue it.
CREATE TABLE queue_message (
    id UUID PRIMARY KEY,
    queue_id UUID NOT NULL REFERENCES queue(id) ON DELETE RESTRICT,
    -- A durable FIFO tie-breaker for messages with the same queue and priority.
    enqueue_order BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    payload BYTEA NOT NULL,
    priority SMALLINT NOT NULL,
    state TEXT NOT NULL DEFAULT 'READY',
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    delivery_attempts SMALLINT NOT NULL DEFAULT 0,
    -- The most recent handle is retained after a lease elapses. Acknowledge
    -- also checks `available_after`, so an old handle cannot acknowledge an
    -- elapsed or subsequently renewed lease.
    receipt_handle UUID,
    -- New messages are immediately visible. Claiming or extending a lease
    -- moves this timestamp into the future.
    available_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_delivered_at TIMESTAMPTZ
);

-- Priorities are deliberately a small fixed set so ordering and metric
-- cardinality stay bounded.
ALTER TABLE queue_message
ADD CONSTRAINT queue_message_priority_range
CHECK (priority BETWEEN 1 AND 3);

-- These are physical row states. Logical readiness additionally depends on
-- `available_after`, `expires_at`, and whether retry attempts remain.
ALTER TABLE queue_message
ADD CONSTRAINT queue_message_state_value
CHECK (state in ('READY', 'IN_FLIGHT'));

-- Defensive lower bound for application and maintenance writes.
ALTER TABLE queue_message
ADD CONSTRAINT queue_message_delivery_attempts_non_negative
CHECK (delivery_attempts >= 0);

-- Every live message has a finite TTL, capped to prevent accidentally
-- unbounded retention.
ALTER TABLE queue_message
ADD CONSTRAINT queue_message_expiration_after_enqueue
CHECK (
    expires_at > enqueued_at
    AND expires_at <= enqueued_at + INTERVAL '30 days'
);

-- READY is reserved for never-delivered rows. Once delivered, a row remains
-- physically IN_FLIGHT across elapsed leases and retries; its handle and
-- delivery history are intentionally retained until acknowledgement,
-- dead-lettering, or expiry removes it.
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

-- Visibility starts no earlier than enqueue, and every delivered lease must
-- end strictly after the delivery that created it.
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

-- Keeps delivery history temporally consistent for retries and DLQ records.
ALTER TABLE queue_message
ADD CONSTRAINT queue_message_delivery_not_before_enqueue
CHECK (
    last_delivered_at IS NULL
    OR last_delivered_at >= enqueued_at
);

-- Supplies every dequeue candidate in one queue-local priority/FIFO order.
-- Dequeue scans this index once and locks only the first eligible row, avoiding
-- separate READY and IN_FLIGHT locks that could hide the unselected row from a
-- concurrent consumer. Lease and expiry eligibility remain residual predicates
-- because their time ranges cannot also provide this delivery order.
CREATE INDEX idx_queue_message_dequeue_claim
ON queue_message (
    queue_id,
    priority DESC,
    enqueue_order ASC
)
WHERE state IN ('READY', 'IN_FLIGHT');

-- Locates elapsed, exhausted leases for dequeue's bounded DLQ maintenance in
-- queue/time/id order. `id` provides deterministic locking for equal times.
CREATE INDEX idx_queue_message_in_flight_dead_letter
ON queue_message (
    queue_id,
    available_after,
    id
)
WHERE state = 'IN_FLIGHT';

-- Supports the TTL worker's global expires-at scan and deterministic row
-- locking. Both permitted states can expire, so a state predicate would not
-- reduce this index.
CREATE INDEX idx_queue_message_ttl_cleanup
ON queue_message (
    expires_at,
    id
);
