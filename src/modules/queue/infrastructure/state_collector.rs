use std::time::Instant;

use anyhow::{Context as _, anyhow};
use sqlx::{PgPool, Postgres, pool::PoolConnection};
use tracing::{Span, field};

use crate::{database, observability::DatabaseMetrics};

use super::super::domain::MessagePriority;

const LOCK_NAMESPACE: i32 = 0x7265_7473; // "rets"
const LOCK_IDENTIFIER: i32 = 0x7153_7461; // "qSta"

#[derive(Clone)]
pub(in crate::modules::queue) struct PostgresQueueStateCollector {
    pool: PgPool,
    metrics: DatabaseMetrics,
}

pub(in crate::modules::queue) struct QueueStateCollectorLease {
    connection: PoolConnection<Postgres>,
}

pub(in crate::modules::queue) struct QueuePriorityStateSnapshot {
    queue_name: String,
    priority: MessagePriority,
    ready: u64,
    in_flight: u64,
    oldest_ready_age_seconds: f64,
    oldest_in_flight_age_seconds: f64,
}

impl QueuePriorityStateSnapshot {
    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn priority(&self) -> MessagePriority {
        self.priority
    }

    pub(in crate::modules::queue) fn ready(&self) -> u64 {
        self.ready
    }

    pub(in crate::modules::queue) fn in_flight(&self) -> u64 {
        self.in_flight
    }

    pub(in crate::modules::queue) fn oldest_ready_age_seconds(&self) -> f64 {
        self.oldest_ready_age_seconds
    }

    pub(in crate::modules::queue) fn oldest_in_flight_age_seconds(&self) -> f64 {
        self.oldest_in_flight_age_seconds
    }
}

impl PostgresQueueStateCollector {
    pub(in crate::modules::queue) fn new(pool: PgPool, metrics: DatabaseMetrics) -> Self {
        Self { pool, metrics }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.try_acquire_state_collector_lease",
            db.pool.acquire.duration = field::Empty,
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    pub(in crate::modules::queue) async fn try_acquire_lease(
        &self,
    ) -> Result<Option<QueueStateCollectorLease>, anyhow::Error> {
        let mut connection = database::acquire(&self.pool, &self.metrics).await?;
        let started = Instant::now();

        let result = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1, $2)")
            .bind(LOCK_NAMESPACE)
            .bind(LOCK_IDENTIFIER)
            .fetch_one(&mut *connection)
            .await;

        self.metrics.operation_finished(
            "queue.try_acquire_state_collector_lease",
            started.elapsed(),
            result.is_ok(),
        );

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        if result? {
            connection.close_on_drop();

            Ok(Some(QueueStateCollectorLease { connection }))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(
        name = "db.operation",
        skip_all,
        fields(
            db.system.name = "postgresql",
            db.operation.name = "queue.read_state_metrics",
            error.type = field::Empty,
            otel.status_code = field::Empty,
        ),
        err
    )]
    pub(in crate::modules::queue) async fn collect(
        &self,
        lease: &mut QueueStateCollectorLease,
    ) -> Result<Vec<QueuePriorityStateSnapshot>, anyhow::Error> {
        let started = Instant::now();

        let result = sqlx::query_as::<_, QueuePriorityStateRow>(
            r#"
            WITH priorities(priority) AS (
                VALUES
                    (3::SMALLINT),
                    (2::SMALLINT),
                    (1::SMALLINT)
            ),
            physical_state AS (
                SELECT
                    state.queue_id,
                    state.priority,
                    SUM(state.ready_count)::BIGINT AS ready,
                    SUM(state.in_flight_count)::BIGINT AS in_flight
                FROM queue_priority_state_shard AS state
                GROUP BY
                    state.queue_id,
                    state.priority
            ),
            expired_ready AS (
                SELECT
                    message.queue_id,
                    message.priority,
                    COUNT(*) AS count
                FROM queue_message AS message
                WHERE message.state = 'READY'
                  AND message.expires_at <= CURRENT_TIMESTAMP
                GROUP BY
                    message.queue_id,
                    message.priority
            ),
            timed_out_in_flight AS (
                SELECT
                    message.queue_id,
                    message.priority,
                    COUNT(*) AS count,
                    COUNT(*) FILTER (
                        WHERE message.expires_at > CURRENT_TIMESTAMP
                          AND message.delivery_attempts
                                < queue.max_delivery_attempts
                    ) AS retryable_count
                FROM queue_message AS message
                JOIN queue
                    ON queue.id = message.queue_id
                WHERE message.state = 'IN_FLIGHT'
                  AND message.available_after <= CURRENT_TIMESTAMP
                GROUP BY
                    message.queue_id,
                    message.priority,
                    queue.max_delivery_attempts
            )
            SELECT
                queue.name AS queue_name,
                priorities.priority,
                COALESCE(
                    physical_state.ready,
                    0
                ) - COALESCE(
                    expired_ready.count,
                    0
                ) + COALESCE(
                    timed_out_in_flight.retryable_count,
                    0
                ) AS ready,
                COALESCE(
                    physical_state.in_flight,
                    0
                ) - COALESCE(
                    timed_out_in_flight.count,
                    0
                ) AS in_flight,
                COALESCE(
                    EXTRACT(
                        EPOCH FROM (
                            CURRENT_TIMESTAMP
                            - oldest_ready.enqueued_at
                        )
                    )::DOUBLE PRECISION,
                    0.0
                ) AS oldest_ready_age_seconds,
                COALESCE(
                    EXTRACT(
                        EPOCH FROM (
                            CURRENT_TIMESTAMP
                            - oldest_in_flight.enqueued_at
                        )
                    )::DOUBLE PRECISION,
                    0.0
                ) AS oldest_in_flight_age_seconds
            FROM queue
            CROSS JOIN priorities
            LEFT JOIN physical_state
                ON physical_state.queue_id = queue.id
               AND physical_state.priority = priorities.priority
            LEFT JOIN expired_ready
                ON expired_ready.queue_id = queue.id
               AND expired_ready.priority = priorities.priority
            LEFT JOIN timed_out_in_flight
                ON timed_out_in_flight.queue_id = queue.id
               AND timed_out_in_flight.priority = priorities.priority
            LEFT JOIN LATERAL (
                SELECT message.enqueued_at
                FROM queue_message AS message
                WHERE message.queue_id = queue.id
                  AND message.priority = priorities.priority
                  AND (
                      (
                          message.state = 'READY'
                          AND message.expires_at > CURRENT_TIMESTAMP
                      )
                      OR (
                          message.state = 'IN_FLIGHT'
                          AND message.available_after <= CURRENT_TIMESTAMP
                          AND message.expires_at > CURRENT_TIMESTAMP
                          AND message.delivery_attempts
                                < queue.max_delivery_attempts
                      )
                  )
                ORDER BY message.enqueued_at
                LIMIT 1
            ) AS oldest_ready
                ON TRUE
            LEFT JOIN LATERAL (
                SELECT message.enqueued_at
                FROM queue_message AS message
                WHERE message.queue_id = queue.id
                  AND message.priority = priorities.priority
                  AND message.state = 'IN_FLIGHT'
                  AND message.available_after > CURRENT_TIMESTAMP
                ORDER BY message.enqueued_at
                LIMIT 1
            ) AS oldest_in_flight
                ON TRUE
            ORDER BY
                queue.name,
                priorities.priority DESC
            "#,
        )
        .fetch_all(&mut *lease.connection)
        .await;

        self.metrics.operation_finished(
            "queue.read_state_metrics",
            started.elapsed(),
            result.is_ok(),
        );

        if let Err(error) = &result {
            Span::current().record("error.type", database::error_type(error));
            Span::current().record("otel.status_code", "ERROR");
        }

        result?
            .into_iter()
            .map(|row| {
                let priority = MessagePriority::from_rank(row.priority).ok_or_else(|| {
                    anyhow!(
                        "queue state query returned invalid priority rank {}",
                        row.priority
                    )
                })?;

                let ready = u64::try_from(row.ready)
                    .context("queue state query returned a negative ready count")?;

                let in_flight = u64::try_from(row.in_flight)
                    .context("queue state query returned a negative in-flight count")?;

                Ok(QueuePriorityStateSnapshot {
                    queue_name: row.queue_name,
                    priority,
                    ready,
                    in_flight,
                    oldest_ready_age_seconds: row.oldest_ready_age_seconds,
                    oldest_in_flight_age_seconds: row.oldest_in_flight_age_seconds,
                })
            })
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct QueuePriorityStateRow {
    queue_name: String,
    priority: i16,
    ready: i64,
    in_flight: i64,
    oldest_ready_age_seconds: f64,
    oldest_in_flight_age_seconds: f64,
}
