use std::time::Instant;

use crate::{database, observability::DatabaseMetrics};

use super::super::{
    application::{
        AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome,
        ExpiredMessagesCleanupSummary, QueueExpiredMessagesCleanupSummary, QueueRepository,
    },
    domain::{Message, MessagePriority, Queue, QueueConfigurationUpdate, QueueDetails},
};

use anyhow::{Context as _, anyhow};
use sqlx::PgPool;
use tracing::{Span, field};
use uuid::Uuid;

const DEQUEUE_DEAD_LETTER_BATCH_SIZE: i64 = 8;

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
    queue_name: Option<String>,
    id: Option<Uuid>,
    payload: Option<Vec<u8>>,
    priority: Option<i16>,
    delivery_attempts: Option<i16>,
    dead_lettered: i64,
}

#[derive(sqlx::FromRow)]
struct ProcessExpiredMessagesRow {
    queue_name: String,
    never_delivered: i64,
    previously_delivered: i64,
}

#[derive(sqlx::FromRow)]
struct QueueDetailsRow {
    id: Uuid,
    name: String,
    visibility_timeout_seconds: i32,
    max_delivery_attempts: i16,
    default_message_ttl_seconds: i32,
}

fn queue_details_from_row(row: QueueDetailsRow) -> Result<QueueDetails, anyhow::Error> {
    let visibility_timeout_seconds = u32::try_from(row.visibility_timeout_seconds)
        .context("stored queue has a negative visibility timeout")?;
    let max_delivery_attempts = u16::try_from(row.max_delivery_attempts)
        .context("stored queue has a negative delivery attempt limit")?;
    let default_message_ttl_seconds = u32::try_from(row.default_message_ttl_seconds)
        .context("stored queue has a negative default message TTL")?;

    Ok(QueueDetails::new(
        row.id,
        row.name,
        visibility_timeout_seconds,
        max_delivery_attempts,
        default_message_ttl_seconds,
    ))
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

        let default_message_ttl_seconds = i32::try_from(queue.default_message_ttl_seconds())
            .expect("validated default message TTL fits in PostgreSQL INTEGER");

        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO queue (
                id,
                name,
                visibility_timeout_seconds,
                max_delivery_attempts,
                default_message_ttl_seconds
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (name) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(queue.id())
        .bind(queue.name())
        .bind(visibility_timeout_seconds)
        .bind(max_delivery_attempts)
        .bind(default_message_ttl_seconds)
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

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.read_name",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;
        let started = Instant::now();

        let result = sqlx::query_scalar::<_, String>(
            r#"
            SELECT name
            FROM queue
            WHERE id = $1
            "#,
        )
        .bind(queue_id)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.read_name", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        Ok(result?)
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.read_details",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn queue_details(&self, queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;
        let started = Instant::now();

        let result = sqlx::query_as::<_, QueueDetailsRow>(
            r#"
            SELECT
                id,
                name,
                visibility_timeout_seconds,
                max_delivery_attempts,
                default_message_ttl_seconds
            FROM queue
            WHERE id = $1
            "#,
        )
        .bind(queue_id)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.read_details", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        result?.map(queue_details_from_row).transpose()
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.update",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn update_queue(
        &self,
        queue_id: Uuid,
        configuration: &QueueConfigurationUpdate,
    ) -> Result<Option<QueueDetails>, anyhow::Error> {
        let visibility_timeout_seconds = configuration.visibility_timeout_seconds().map(|value| {
            i32::try_from(value).expect("validated visibility timeout fits PostgreSQL INTEGER")
        });
        let max_delivery_attempts = configuration.max_delivery_attempts().map(|value| {
            i16::try_from(value).expect("validated delivery attempt limit fits PostgreSQL SMALLINT")
        });
        let default_message_ttl_seconds =
            configuration.default_message_ttl_seconds().map(|value| {
                i32::try_from(value).expect("validated message TTL fits PostgreSQL INTEGER")
            });

        let mut connection = database::acquire(&self.pool, &self.metrics).await?;
        let started = Instant::now();

        let result = sqlx::query_as::<_, QueueDetailsRow>(
            r#"
            UPDATE queue
            SET
                visibility_timeout_seconds = COALESCE(
                    $2,
                    visibility_timeout_seconds
                ),
                max_delivery_attempts = COALESCE(
                    $3,
                    max_delivery_attempts
                ),
                default_message_ttl_seconds = COALESCE(
                    $4,
                    default_message_ttl_seconds
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING
                id,
                name,
                visibility_timeout_seconds,
                max_delivery_attempts,
                default_message_ttl_seconds
            "#,
        )
        .bind(queue_id)
        .bind(visibility_timeout_seconds)
        .bind(max_delivery_attempts)
        .bind(default_message_ttl_seconds)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.update", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        result?.map(queue_details_from_row).transpose()
    }

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
        queue_id: Uuid,
        message: &Message,
        effective_ttl_seconds: u32,
    ) -> Result<(), anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query(
            r#"
            INSERT INTO queue_message (
                id, queue_id, payload, priority, expires_at
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                CURRENT_TIMESTAMP + ($5::BIGINT * INTERVAL '1 second')
            )
            "#,
        )
        .bind(message.id())
        .bind(queue_id)
        .bind(message.payload().as_bytes())
        .bind(message.priority().rank())
        .bind(i64::from(effective_ttl_seconds))
        .execute(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.enqueue", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        result?;
        Ok(())
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
        queue_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_as::<_, DequeueMessageRow>(
            r#"
            WITH target_queue AS MATERIALIZED (
                SELECT
                    id,
                    name,
                    visibility_timeout_seconds,
                    max_delivery_attempts
                FROM queue
                WHERE id = $1
            ),
            dead_letter_candidates AS MATERIALIZED (
                SELECT message.id
                FROM queue_message AS message
                JOIN target_queue
                    ON target_queue.id = message.queue_id
                WHERE message.state = 'IN_FLIGHT'
                  AND message.available_after <= CURRENT_TIMESTAMP
                  AND message.expires_at > CURRENT_TIMESTAMP
                  AND message.delivery_attempts
                        >= target_queue.max_delivery_attempts
                ORDER BY
                    message.available_after ASC,
                    message.id ASC
                FOR UPDATE OF message SKIP LOCKED
                LIMIT $3
            ),
            dead_lettered AS (
                DELETE FROM queue_message AS message
                USING dead_letter_candidates AS candidate
                WHERE message.id = candidate.id
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
                RETURNING id
            ),
            candidate AS MATERIALIZED (
                SELECT
                    message.id,
                    message.priority,
                    message.enqueue_order,
                    target_queue.visibility_timeout_seconds
                FROM queue_message AS message
                JOIN target_queue
                    ON target_queue.id = message.queue_id
                WHERE message.state IN ('READY', 'IN_FLIGHT')
                  AND message.expires_at > CURRENT_TIMESTAMP
                  AND message.delivery_attempts
                        < target_queue.max_delivery_attempts
                  AND (
                      message.state = 'READY'
                      OR message.available_after <= CURRENT_TIMESTAMP
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
                    available_after =
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
                (
                    SELECT name
                    FROM target_queue
                ) AS queue_name,
                leased.id,
                leased.payload,
                leased.priority,
                leased.delivery_attempts,
                (
                    SELECT COUNT(*)
                    FROM stored_dead_letters
                ) AS dead_lettered
            FROM (VALUES (1)) AS sentinel(value)
            LEFT JOIN leased ON TRUE
            "#,
        )
        .bind(queue_id)
        .bind(receipt_handle)
        .bind(DEQUEUE_DEAD_LETTER_BATCH_SIZE)
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

        let queue_name = row
            .queue_name
            .ok_or_else(|| anyhow!("dequeue query omitted the target queue name"))?;
        let dead_lettered = u64::try_from(row.dead_lettered)
            .context("dequeue query returned a negative dead-letter count")?;

        match (row.id, row.payload, row.priority, row.delivery_attempts) {
            (None, None, None, None) => Ok(DequeueMessageOutcome::Empty {
                queue_name,
                dead_lettered,
            }),
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
                    queue_name,
                    dead_lettered,
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
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            DELETE FROM queue_message
            WHERE id = $1
              AND queue_id = $2
              AND receipt_handle = $3
              AND available_after > CURRENT_TIMESTAMP
            RETURNING id
            "#,
        )
        .bind(message_id)
        .bind(queue_id)
        .bind(receipt_handle)
        .fetch_optional(&mut *connection)
        .await;

        self.metrics
            .operation_finished("queue.acknowledge", started.elapsed(), result.is_ok());

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        match result? {
            Some(_) => Ok(AcknowledgeMessageOutcome::Acknowledged),
            None => Ok(AcknowledgeMessageOutcome::Unchanged),
        }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.process_expired_messages",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;

        let started = Instant::now();

        let result = sqlx::query_as::<_, ProcessExpiredMessagesRow>(
            r#"
            WITH expired AS MATERIALIZED (
                SELECT message.id
                FROM queue_message AS message
                WHERE message.expires_at <= CURRENT_TIMESTAMP
                  AND (
                      message.state = 'READY'
                      OR (
                          message.state = 'IN_FLIGHT'
                          AND message.available_after <= CURRENT_TIMESTAMP
                      )
                  )
                ORDER BY
                    message.expires_at ASC,
                    message.id ASC
                FOR UPDATE OF message SKIP LOCKED
                LIMIT $1
            ),
            deleted AS (
                DELETE FROM queue_message AS message
                USING expired
                WHERE message.id = expired.id
                RETURNING
                    message.queue_id,
                    message.delivery_attempts
            )
            SELECT
                queue.name AS queue_name,
                COUNT(*) FILTER (
                    WHERE deleted.delivery_attempts = 0
                ) AS never_delivered,
                COUNT(*) FILTER (
                    WHERE deleted.delivery_attempts > 0
                ) AS previously_delivered
            FROM deleted
            JOIN queue
                ON queue.id = deleted.queue_id
            GROUP BY queue.name
            ORDER BY queue.name
            "#,
        )
        .bind(i64::from(batch_size))
        .fetch_all(&mut *connection)
        .await;

        self.metrics.operation_finished(
            "queue.process_expired_messages",
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
                |row| -> Result<QueueExpiredMessagesCleanupSummary, anyhow::Error> {
                    let never_delivered = u64::try_from(row.never_delivered).context(
                        "expired message query returned a negative never-delivered count",
                    )?;

                    let previously_delivered = u64::try_from(row.previously_delivered).context(
                        "expired message query returned a negative previously-delivered count",
                    )?;

                    Ok(QueueExpiredMessagesCleanupSummary::new(
                        row.queue_name,
                        never_delivered,
                        previously_delivered,
                    ))
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ExpiredMessagesCleanupSummary::new(per_queue))
    }
}
