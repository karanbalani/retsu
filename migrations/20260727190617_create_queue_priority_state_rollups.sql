-- Stores sharded counts of the physical READY/IN_FLIGHT values written to
-- `queue_message`. PostgreSQL triggers only observe explicit DML, so elapsed
-- leases and expired messages remain in their last physical state here.
--
-- The state collector derives the consumer-visible counts from this rollup:
--   logical ready
--     = stored ready
--     - expired READY rows
--     + unexpired, retryable IN_FLIGHT rows whose lease has elapsed
--   logical in flight
--     = stored in flight
--     - IN_FLIGHT rows whose lease has elapsed
-- An elapsed lease that is expired or attempt-exhausted is included in neither
-- logical count. This hybrid avoids both full-table COUNT scans and timeout
-- requeue writes. Thirty-two deterministic shards spread hot-queue counter
-- updates across separate rows while keeping collector aggregation bounded.
CREATE TABLE queue_priority_state_shard (
    queue_id UUID NOT NULL REFERENCES queue(id) ON DELETE CASCADE,
    priority SMALLINT NOT NULL,
    shard SMALLINT NOT NULL,
    ready_count BIGINT NOT NULL,
    in_flight_count BIGINT NOT NULL
);

-- One counter row exists for each queue, priority, and deterministic shard.
ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_primary_key
PRIMARY KEY (queue_id, priority, shard);

-- Mirrors the bounded priority domain enforced on `queue_message`.
ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_priority_range
CHECK (priority BETWEEN 1 AND 3);

-- The shard count is part of the persisted layout, not runtime configuration.
ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_number_range
CHECK (shard BETWEEN 0 AND 31);

-- Negative physical counts indicate rollup drift, so fail the message
-- transaction instead of hiding an invalid metric.
ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_ready_count_non_negative
CHECK (ready_count >= 0);

-- Apply the same drift guard independently to the physical in-flight count.
ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_in_flight_count_non_negative
CHECK (in_flight_count >= 0);

-- Select a stable shard from the message UUID. The same message therefore
-- decrements the exact shard that was incremented when it was inserted, while
-- the UUID's random bits distribute concurrent writes across 32 rows.
CREATE FUNCTION queue_message_state_shard(message_id UUID)
RETURNS SMALLINT
LANGUAGE SQL
IMMUTABLE
STRICT
PARALLEL SAFE
AS $$
    SELECT (get_byte(uuid_send(message_id), 15) % 32)::SMALLINT
$$;

-- Update an existing shard first for the common path. Missing positive shards
-- are created safely under concurrency; a missing negative shard is rejected
-- because it would prove that the rollup and source table disagree.
CREATE FUNCTION adjust_queue_priority_state(
    target_queue_id UUID,
    target_priority SMALLINT,
    target_shard SMALLINT,
    ready_delta BIGINT,
    in_flight_delta BIGINT
)
RETURNS VOID
LANGUAGE PLPGSQL
AS $$
BEGIN
    UPDATE queue_priority_state_shard
    SET
        ready_count = ready_count + ready_delta,
        in_flight_count = in_flight_count + in_flight_delta
    WHERE queue_id = target_queue_id
      AND priority = target_priority
      AND shard = target_shard;

    IF FOUND THEN
        RETURN;
    END IF;

    IF ready_delta < 0 OR in_flight_delta < 0 THEN
        RAISE EXCEPTION
            'cannot decrement missing queue priority state shard'
            USING ERRCODE = 'check_violation';
    END IF;

    INSERT INTO queue_priority_state_shard (
        queue_id,
        priority,
        shard,
        ready_count,
        in_flight_count
    )
    VALUES (
        target_queue_id,
        target_priority,
        target_shard,
        ready_delta,
        in_flight_delta
    )
    ON CONFLICT (queue_id, priority, shard)
    DO UPDATE SET
        ready_count =
            queue_priority_state_shard.ready_count
            + EXCLUDED.ready_count,
        in_flight_count =
            queue_priority_state_shard.in_flight_count
            + EXCLUDED.in_flight_count;
END;
$$;

-- Statement-level transition tables aggregate a multi-row message change into
-- at most one adjustment per affected shard. The ordered aggregate also gives
-- concurrent transactions a consistent shard-lock order.
CREATE FUNCTION maintain_queue_priority_state()
RETURNS TRIGGER
LANGUAGE PLPGSQL
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM adjust_queue_priority_state(
            change.queue_id,
            change.priority,
            change.shard,
            change.ready_delta,
            change.in_flight_delta
        )
        FROM (
            SELECT
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id) AS shard,
                COUNT(*) FILTER (
                    WHERE message.state = 'READY'
                ) AS ready_delta,
                COUNT(*) FILTER (
                    WHERE message.state = 'IN_FLIGHT'
                ) AS in_flight_delta
            FROM new_messages AS message
            GROUP BY
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id)
            ORDER BY
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id)
        ) AS change;

    ELSIF TG_OP = 'DELETE' THEN
        PERFORM adjust_queue_priority_state(
            change.queue_id,
            change.priority,
            change.shard,
            change.ready_delta,
            change.in_flight_delta
        )
        FROM (
            SELECT
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id) AS shard,
                -(COUNT(*) FILTER (
                    WHERE message.state = 'READY'
                )) AS ready_delta,
                -(COUNT(*) FILTER (
                    WHERE message.state = 'IN_FLIGHT'
                )) AS in_flight_delta
            FROM old_messages AS message
            GROUP BY
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id)
            ORDER BY
                message.queue_id,
                message.priority,
                queue_message_state_shard(message.id)
        ) AS change;

    ELSIF TG_OP = 'UPDATE' THEN
        -- Lease retry/extension updates change handles, attempts, and times but
        -- not physical queue/priority/state counts. Exit before touching the
        -- rollup on that high-frequency path.
        IF NOT EXISTS (
            SELECT 1
            FROM old_messages AS old_message
            FULL JOIN new_messages AS new_message
                ON new_message.id = old_message.id
            WHERE old_message.id IS NULL
               OR new_message.id IS NULL
               OR old_message.queue_id IS DISTINCT FROM new_message.queue_id
               OR old_message.priority IS DISTINCT FROM new_message.priority
               OR old_message.state IS DISTINCT FROM new_message.state
        ) THEN
            RETURN NULL;
        END IF;

        PERFORM adjust_queue_priority_state(
            change.queue_id,
            change.priority,
            change.shard,
            change.ready_delta,
            change.in_flight_delta
        )
        FROM (
            WITH changes AS (
                SELECT
                    message.queue_id,
                    message.priority,
                    queue_message_state_shard(message.id) AS shard,
                    -(COUNT(*) FILTER (
                        WHERE message.state = 'READY'
                    )) AS ready_delta,
                    -(COUNT(*) FILTER (
                        WHERE message.state = 'IN_FLIGHT'
                    )) AS in_flight_delta
                FROM old_messages AS message
                GROUP BY
                    message.queue_id,
                    message.priority,
                    queue_message_state_shard(message.id)

                UNION ALL

                SELECT
                    message.queue_id,
                    message.priority,
                    queue_message_state_shard(message.id) AS shard,
                    COUNT(*) FILTER (
                        WHERE message.state = 'READY'
                    ) AS ready_delta,
                    COUNT(*) FILTER (
                        WHERE message.state = 'IN_FLIGHT'
                    ) AS in_flight_delta
                FROM new_messages AS message
                GROUP BY
                    message.queue_id,
                    message.priority,
                    queue_message_state_shard(message.id)
            )
            SELECT
                queue_id,
                priority,
                shard,
                SUM(ready_delta)::BIGINT AS ready_delta,
                SUM(in_flight_delta)::BIGINT AS in_flight_delta
            FROM changes
            GROUP BY
                queue_id,
                priority,
                shard
            HAVING
                SUM(ready_delta) <> 0
                OR SUM(in_flight_delta) <> 0
            ORDER BY
                queue_id,
                priority,
                shard
        ) AS change;
    END IF;

    RETURN NULL;
END;
$$;

-- Transition-table triggers keep rollups transactional for every writer,
-- including direct SQL and future application entry points. If the message
-- statement rolls back, its counter adjustment rolls back with it.
CREATE TRIGGER queue_message_state_after_insert
AFTER INSERT ON queue_message
REFERENCING NEW TABLE AS new_messages
FOR EACH STATEMENT
EXECUTE FUNCTION maintain_queue_priority_state();

CREATE TRIGGER queue_message_state_after_delete
AFTER DELETE ON queue_message
REFERENCING OLD TABLE AS old_messages
FOR EACH STATEMENT
EXECUTE FUNCTION maintain_queue_priority_state();

CREATE TRIGGER queue_message_state_after_update
AFTER UPDATE ON queue_message
REFERENCING
    OLD TABLE AS old_messages
    NEW TABLE AS new_messages
FOR EACH STATEMENT
EXECUTE FUNCTION maintain_queue_priority_state();

-- Install the triggers before backfilling so writes cannot commit without a
-- matching rollup adjustment during migration.
INSERT INTO queue_priority_state_shard (
    queue_id,
    priority,
    shard,
    ready_count,
    in_flight_count
)
SELECT
    message.queue_id,
    message.priority,
    queue_message_state_shard(message.id),
    COUNT(*) FILTER (
        WHERE message.state = 'READY'
    ) AS ready_count,
    COUNT(*) FILTER (
        WHERE message.state = 'IN_FLIGHT'
    ) AS in_flight_count
FROM queue_message AS message
GROUP BY
    message.queue_id,
    message.priority,
    queue_message_state_shard(message.id);

-- Finds READY rows that have expired since the physical rollup was updated.
-- Leading with `expires_at` makes the collector scan only the elapsed time
-- range, then group corrections by queue and priority.
CREATE INDEX idx_queue_message_ready_expires_at_queue_id_priority
ON queue_message (
    expires_at,
    queue_id,
    priority
)
WHERE state = 'READY';

-- Finds elapsed IN_FLIGHT leases for both logical count corrections. Included
-- attempts and expiry let the collector distinguish retryable, exhausted, and
-- expired rows without fetching those columns solely for the predicates.
CREATE INDEX idx_queue_message_in_flight_available_after_queue_id_priority
ON queue_message (
    available_after,
    queue_id,
    priority
)
INCLUDE (
    delivery_attempts,
    expires_at
)
WHERE state = 'IN_FLIGHT';

-- Supports queue/priority-local oldest READY age seeks ordered by enqueue time.
-- Included expiry lets the collector skip logically expired candidates.
CREATE INDEX idx_queue_message_ready_queue_id_priority_enqueued_at
ON queue_message (
    queue_id,
    priority,
    enqueued_at
)
INCLUDE (expires_at)
WHERE state = 'READY';

-- Supports oldest-message seeks across physical IN_FLIGHT rows in
-- queue/priority/enqueue-time order. Included lease, attempt, and expiry data
-- cover both retryable-as-ready and actively leased eligibility checks.
CREATE INDEX idx_queue_message_in_flight_queue_id_priority_enqueued_at
ON queue_message (
    queue_id,
    priority,
    enqueued_at
)
INCLUDE (
    available_after,
    delivery_attempts,
    expires_at,
    enqueue_order
)
WHERE state = 'IN_FLIGHT';
