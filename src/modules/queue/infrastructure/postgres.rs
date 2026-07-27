use std::time::Instant;

use crate::{database, observability::DatabaseMetrics};

use super::super::{
    application::{
        AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome,
        EnqueueMessageOutcome, MessageRepository, QueueRepository, QueueTimeoutProcessingSummary,
        TimeoutProcessingSummary,
    },
    domain::{Message, MessagePriority, Queue},
};

use anyhow::{Context as _, anyhow};
use sqlx::PgPool;
use tracing::{Span, field};
use uuid::Uuid;

#[derive(Clone)]
pub(in crate::modules::queue) struct PostgresQueueRepository {
    pool: PgPool,
    metrics: DatabaseMetrics,
}

impl PostgresQueueRepository {
    pub(in crate::modules::queue) fn new(pool: PgPool, metrics: DatabaseMetrics) -> Self {
        Self { pool, metrics }
    }
}

#[derive(sqlx::FromRow)]
struct DequeueMessageRow {
    queue_exists: bool,
    id: Option<Uuid>,
    payload: Option<Vec<u8>>,
    priority: Option<i16>,
    delivery_attempts: Option<i16>,
}

#[derive(sqlx::FromRow)]
struct AcknowledgeMessageRow {
    queue_exists: bool,
    message_exists: bool,
    acknowledged: bool,
}

#[derive(sqlx::FromRow)]
struct ProcessTimedOutMessagesRow {
    queue_name: String,
    requeued: i64,
    dead_lettered: i64,
}

impl QueueRepository for PostgresQueueRepository {
    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.create",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        let visibility_timeout_seconds = i32::try_from(queue.visibility_timeout_seconds())
            .expect("validated visibility timeout fits in PostgreSQL INTEGER");

        let max_delivery_attempts = i16::try_from(queue.max_delivery_attempts())
            .expect("validated delivery attempt limit fits in PostgreSQL SMALLINT");

        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO queue (
                id, name, visibility_timeout_seconds, max_delivery_attempts
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (name) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(queue.id())
        .bind(queue.name())
        .bind(visibility_timeout_seconds)
        .bind(max_delivery_attempts)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.create", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        let inserted_id = result?;
        match inserted_id {
            Some(_) => Ok(CreateQueueOutcome::Created),
            None => Ok(CreateQueueOutcome::AlreadyExists),
        }
    }
}

impl MessageRepository for PostgresQueueRepository {
    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.enqueue",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn enqueue_message(
        &self,
        queue_name: &str,
        message: &Message,
    ) -> Result<EnqueueMessageOutcome, anyhow::Error> {
        let ttl_seconds = message.ttl_seconds().map(i64::from);

        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO queue_message (
                id, queue_id, payload, priority, expires_at
            )
            SELECT
                $1,
                queue.id,
                $2,
                $3,
                CASE
                    WHEN $4::BIGINT IS NULL THEN NULL
                    ELSE CURRENT_TIMESTAMP + ($4::BIGINT * INTERVAL '1 second')
                END
            FROM queue
            WHERE queue.name = $5
            RETURNING id
            "#,
        )
        .bind(message.id())
        .bind(message.payload().as_bytes())
        .bind(message.priority().rank())
        .bind(ttl_seconds)
        .bind(queue_name)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.enqueue", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        let inserted_id = result?;
        match inserted_id {
            Some(_) => Ok(EnqueueMessageOutcome::Enqueued),
            None => Ok(EnqueueMessageOutcome::QueueNotFound),
        }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.dequeue",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn dequeue_message(
        &self,
        queue_name: &str,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_as::<_, DequeueMessageRow>(
            r#"
            WITH target_queue AS MATERIALIZED (
                SELECT id, visibility_timeout_seconds
                FROM queue
                WHERE name = $1
            ),
            candidate AS (
                SELECT
                    message.id,
                    target_queue.visibility_timeout_seconds
                FROM queue_message AS message
                JOIN target_queue
                    ON target_queue.id = message.queue_id
                WHERE message.state = 'READY'
                  AND (
                      message.expires_at IS NULL
                      OR message.expires_at > CURRENT_TIMESTAMP
                  )
                ORDER BY
                    message.priority DESC,
                    message.enqueue_order ASC
                FOR UPDATE OF message SKIP LOCKED
                LIMIT 1
            ),
            leased AS (
                UPDATE queue_message AS message
                SET
                    state = 'IN_FLIGHT',
                    receipt_handle = $2,
                    delivery_attempts = message.delivery_attempts + 1,
                    last_delivered_at = CURRENT_TIMESTAMP,
                    visibility_deadline =
                        CURRENT_TIMESTAMP
                        + (
                            candidate.visibility_timeout_seconds
                            * INTERVAL '1 second'
                        )
                FROM candidate
                WHERE message.id = candidate.id
                RETURNING
                    message.id,
                    message.payload,
                    message.priority,
                    message.delivery_attempts
            )
            SELECT
                EXISTS (
                    SELECT 1
                    FROM target_queue
                ) AS queue_exists,
                leased.id,
                leased.payload,
                leased.priority,
                leased.delivery_attempts
            FROM (VALUES (1)) AS sentinel(value)
            LEFT JOIN leased ON TRUE
            "#,
        )
        .bind(queue_name)
        .bind(receipt_handle)
        .fetch_one(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.dequeue", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        let row = result?;
        if !row.queue_exists {
            return Ok(DequeueMessageOutcome::QueueNotFound);
        }

        match (row.id, row.payload, row.priority, row.delivery_attempts) {
            (None, None, None, None) => Ok(DequeueMessageOutcome::Empty),
            (Some(id), Some(payload), Some(priority_rank), Some(delivery_attempts)) => {
                let payload = String::from_utf8(payload)
                    .context("stored queue message payload is not valid UTF-8")?;

                let priority = MessagePriority::from_rank(priority_rank).ok_or_else(|| {
                    anyhow!("stored queue message has invalid priority rank {priority_rank}")
                })?;

                let delivery_attempts = u16::try_from(delivery_attempts)
                    .context("stored queue message has a negative delivery attempt count")?;

                Ok(DequeueMessageOutcome::Dequeued {
                    id,
                    payload,
                    priority,
                    receipt_handle,
                    delivery_attempts,
                })
            }
            _ => Err(anyhow!(
                "dequeue query returned an incomplete leased message"
            )),
        }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.acknowledge",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn acknowledge_message(
        &self,
        queue_name: &str,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_as::<_, AcknowledgeMessageRow>(
            r#"
            WITH target_queue AS MATERIALIZED (
                SELECT id
                FROM queue
                WHERE name = $1
            ),
            target_message AS MATERIALIZED (
                SELECT message.id
                FROM queue_message AS message
                JOIN target_queue
                    ON target_queue.id = message.queue_id
                WHERE message.id = $2
            ),
            acknowledged AS (
                DELETE FROM queue_message AS message
                USING target_queue
                WHERE message.queue_id = target_queue.id
                    AND message.id = $2
                    AND message.state = 'IN_FLIGHT'
                    AND message.receipt_handle = $3
                    AND message.visibility_deadline > CURRENT_TIMESTAMP
                RETURNING message.id
            )
            SELECT
                EXISTS (
                    SELECT 1
                    FROM target_queue
                ) AS queue_exists,
                EXISTS (
                    SELECT 1
                    FROM target_message
                ) AS message_exists,
                EXISTS (
                    SELECT 1
                    FROM acknowledged
                ) AS acknowledged
            "#,
        )
        .bind(queue_name)
        .bind(message_id)
        .bind(receipt_handle)
        .fetch_one(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.acknowledge", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        let row = result?;

        match (row.queue_exists, row.message_exists, row.acknowledged) {
            (_, _, true) => Ok(AcknowledgeMessageOutcome::Acknowledged),

            (false, _, false) => Ok(AcknowledgeMessageOutcome::QueueNotFound),

            (true, false, false) => Ok(AcknowledgeMessageOutcome::MessageNotFound),

            (true, true, false) => Ok(AcknowledgeMessageOutcome::ReceiptHandleInvalid),
        }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.process_timed_out_messages",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn process_timed_out_messages(
        &self,
        batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_as::<_, ProcessTimedOutMessagesRow>(
            r#"
            WITH timed_out AS MATERIALIZED (
                SELECT
                    message.id,
                    queue.max_delivery_attempts
                FROM queue_message AS message
                JOIN queue
                    ON queue.id = message.queue_id
                WHERE message.state = 'IN_FLIGHT'
                  AND message.visibility_deadline <= CURRENT_TIMESTAMP
                ORDER BY
                    message.visibility_deadline ASC,
                    message.id ASC
                FOR UPDATE OF message SKIP LOCKED
                LIMIT $1
            ),
            dead_lettered AS (
                DELETE FROM queue_message AS message
                USING timed_out
                WHERE message.id = timed_out.id
                  AND message.delivery_attempts >= timed_out.max_delivery_attempts
                RETURNING
                    message.id,
                    message.queue_id,
                    message.payload,
                    message.priority,
                    message.enqueued_at,
                    message.expires_at,
                    message.delivery_attempts,
                    message.last_delivered_at
            ),
            stored_dead_letters AS (
                INSERT INTO queue_dead_letter_message (
                    id,
                    queue_id,
                    payload,
                    priority,
                    enqueued_at,
                    expires_at,
                    delivery_attempts,
                    last_delivered_at,
                    dead_lettered_at,
                    reason
                )
                SELECT
                    dead_lettered.id,
                    dead_lettered.queue_id,
                    dead_lettered.payload,
                    dead_lettered.priority,
                    dead_lettered.enqueued_at,
                    dead_lettered.expires_at,
                    dead_lettered.delivery_attempts,
                    dead_lettered.last_delivered_at,
                    CURRENT_TIMESTAMP,
                    'MAX_DELIVERY_ATTEMPTS_EXHAUSTED'
                FROM dead_lettered
                RETURNING queue_id
            ),
            requeued AS (
                UPDATE queue_message AS message
                SET
                    state = 'READY',
                    receipt_handle = NULL,
                    visibility_deadline = NULL
                FROM timed_out
                WHERE message.id = timed_out.id
                  AND message.delivery_attempts < timed_out.max_delivery_attempts
                RETURNING message.queue_id
            ),
            affected AS (
                SELECT
                    queue_id,
                    FALSE AS was_dead_lettered
                FROM requeued

                UNION ALL

                SELECT
                    queue_id,
                    TRUE AS was_dead_lettered
                FROM stored_dead_letters
            )
            SELECT
                queue.name AS queue_name,
                COUNT(*) FILTER (
                    WHERE NOT affected.was_dead_lettered
                ) AS requeued,
                COUNT(*) FILTER (
                    WHERE affected.was_dead_lettered
                ) AS dead_lettered
            FROM affected
            JOIN queue
                ON queue.id = affected.queue_id
            GROUP BY queue.name
            ORDER BY queue.name
            "#,
        )
        .bind(i64::from(batch_size))
        .fetch_all(&mut *connection)
        .await;

        self.metrics.operation_finished(
            "queue.process_timed_out_messages",
            started.elapsed(),
            result.is_ok(),
        );

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        let per_queue = result?
            .into_iter()
            .map(
                |row| -> Result<QueueTimeoutProcessingSummary, anyhow::Error> {
                    let requeued = u64::try_from(row.requeued)
                        .context("timed-out message query returned a negative requeue count")?;

                    let dead_lettered = u64::try_from(row.dead_lettered)
                        .context("timed-out message query returned a negative dead-letter count")?;

                    Ok(QueueTimeoutProcessingSummary::new(
                        row.queue_name,
                        requeued,
                        dead_lettered,
                    ))
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TimeoutProcessingSummary::new(per_queue))
    }
}
