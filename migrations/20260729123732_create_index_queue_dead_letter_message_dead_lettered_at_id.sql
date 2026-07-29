-- no-transaction
-- Supports the DLQ retention worker's global oldest-first scan. `id` provides
-- deterministic locking when a batch of messages shares `dead_lettered_at`.
CREATE INDEX CONCURRENTLY idx_queue_dead_letter_message_dead_lettered_at_id
ON queue_dead_letter_message (
    dead_lettered_at ASC,
    id ASC
);
