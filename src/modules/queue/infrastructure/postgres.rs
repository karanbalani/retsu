use std::time::Instant;

use crate::{database, observability::DatabaseMetrics};

use super::super::{
    application::{CreateQueueOutcome, EnqueueMessageOutcome, MessageRepository, QueueRepository},
    domain::{Message, Queue},
};

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
}
