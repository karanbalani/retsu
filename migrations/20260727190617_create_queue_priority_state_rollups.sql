CREATE TABLE queue_priority_state_shard (
    queue_id UUID NOT NULL REFERENCES queue(id) ON DELETE CASCADE,
    priority SMALLINT NOT NULL,
    shard SMALLINT NOT NULL,
    ready_count BIGINT NOT NULL,
    in_flight_count BIGINT NOT NULL
);

ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_primary_key
PRIMARY KEY (queue_id, priority, shard);

ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_priority_range
CHECK (priority BETWEEN 1 AND 3);

ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_number_range
CHECK (shard BETWEEN 0 AND 31);

ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_ready_count_non_negative
CHECK (ready_count >= 0);

ALTER TABLE queue_priority_state_shard
ADD CONSTRAINT queue_priority_state_shard_in_flight_count_non_negative
CHECK (in_flight_count >= 0);

CREATE FUNCTION queue_message_state_shard(message_id UUID)
RETURNS SMALLINT
LANGUAGE SQL
IMMUTABLE
STRICT
PARALLEL SAFE
AS $$
    SELECT (get_byte(uuid_send(message_id), 15) % 32)::SMALLINT
$$;

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

CREATE INDEX idx_queue_message_ready_expires_at_queue_id_priority
ON queue_message (
    expires_at,
    queue_id,
    priority
)
WHERE state = 'READY';

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

DROP INDEX idx_queue_message_expired_leases;

CREATE INDEX idx_queue_message_in_flight_queue_id_available_after_id
ON queue_message (
    queue_id,
    available_after,
    id
)
INCLUDE (
    delivery_attempts,
    expires_at
)
WHERE state = 'IN_FLIGHT';

CREATE INDEX idx_queue_message_ready_queue_id_priority_enqueued_at
ON queue_message (
    queue_id,
    priority,
    enqueued_at
)
INCLUDE (expires_at)
WHERE state = 'READY';

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

CREATE INDEX idx_queue_message_in_flight_retry_dequeue
ON queue_message (
    queue_id,
    priority DESC,
    enqueue_order ASC
)
INCLUDE (
    available_after,
    delivery_attempts,
    expires_at
)
WHERE state = 'IN_FLIGHT';
