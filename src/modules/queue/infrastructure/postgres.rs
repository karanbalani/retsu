use std::time::Instant;

use crate::{database, observability::DatabaseMetrics};

use super::super::{
    application::{
        CreateQueueOutcome, DequeueMessageOutcome, EnqueueMessageOutcome, MessageRepository,
        QueueRepository,
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
}
