use std::time::Duration;

use anyhow::Context as _;
use sqlx::PgPool;
use uuid::Uuid;

use super::PostgresQueueRepository;
use crate::{
    modules::queue::{
        application::{MessageRepository, QueuePriorityStateSnapshot, QueueStateRepository},
        domain::MessagePriority,
    },
    observability::test_metrics,
};

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requires a PostgreSQL server"]
async fn state_collector_lease_allows_one_holder_and_releases_on_drop(
    pool: PgPool,
) -> Result<(), anyhow::Error> {
    let (provider, metrics) = test_metrics();
    let repository = PostgresQueueRepository::new(pool, metrics.database().clone());

    let first = repository
        .try_acquire_collector_lease()
        .await?
        .context("the first collector should acquire leadership")?;

    assert!(
        repository.try_acquire_collector_lease().await?.is_none(),
        "a second collector should remain on standby"
    );

    drop(first);

    let second = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(lease) = repository.try_acquire_collector_lease().await? {
                return Ok::<_, anyhow::Error>(lease);
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("collector leadership should be released after the holder is dropped")??;

    drop(second);

    provider.shutdown().expect("provider should shut down");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requires a PostgreSQL server"]
async fn processing_timeouts_requeues_retryable_and_dead_letters_exhausted_messages(
    pool: PgPool,
) -> Result<(), anyhow::Error> {
    let queue_id = Uuid::now_v7();
    let retryable_id = Uuid::now_v7();
    let exhausted_id = Uuid::now_v7();
    let retryable_receipt_handle = Uuid::new_v4();
    let exhausted_receipt_handle = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO queue (
            id,
            name,
            visibility_timeout_seconds,
            max_delivery_attempts,
            default_message_ttl_seconds
        )
        VALUES ($1, 'email-delivery', 30, 2, 3600)
        "#,
    )
    .bind(queue_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO queue_message (
            id,
            queue_id,
            payload,
            priority,
            state,
            enqueued_at,
            expires_at,
            delivery_attempts,
            receipt_handle,
            visibility_deadline,
            last_delivered_at
        )
        VALUES
            (
                $1,
                $3,
                $4,
                3,
                'IN_FLIGHT',
                CURRENT_TIMESTAMP - INTERVAL '10 minutes',
                CURRENT_TIMESTAMP + INTERVAL '1 hour',
                1,
                $6,
                CURRENT_TIMESTAMP - INTERVAL '1 minute',
                CURRENT_TIMESTAMP - INTERVAL '2 minutes'
            ),
            (
                $2,
                $3,
                $5,
                2,
                'IN_FLIGHT',
                CURRENT_TIMESTAMP - INTERVAL '10 minutes',
                CURRENT_TIMESTAMP + INTERVAL '1 hour',
                2,
                $7,
                CURRENT_TIMESTAMP - INTERVAL '1 minute',
                CURRENT_TIMESTAMP - INTERVAL '2 minutes'
            )
        "#,
    )
    .bind(retryable_id)
    .bind(exhausted_id)
    .bind(queue_id)
    .bind(b"retryable-payload".as_slice())
    .bind(b"exhausted-payload".as_slice())
    .bind(retryable_receipt_handle)
    .bind(exhausted_receipt_handle)
    .execute(&pool)
    .await?;

    let (provider, metrics) = test_metrics();
    let repository = PostgresQueueRepository::new(pool.clone(), metrics.database().clone());
    let mut lease = repository
        .try_acquire_collector_lease()
        .await?
        .context("the lifecycle test should acquire collector leadership")?;

    let state_before_processing = repository.queue_state(&mut lease).await?;
    assert_eq!(state_before_processing.len(), 3);

    for priority in [
        MessagePriority::High,
        MessagePriority::Medium,
        MessagePriority::Low,
    ] {
        let state = state_for(&state_before_processing, priority);

        assert_eq!(state.ready(), 0);
        assert_eq!(
            state.in_flight(),
            0,
            "timed-out deliveries should not appear in the logical in-flight state"
        );
    }

    let summary = repository.process_timed_out_messages(500).await?;

    let queue_summary = summary
        .per_queue()
        .first()
        .expect("the affected queue should be summarized");
    assert_eq!(summary.per_queue().len(), 1);
    assert_eq!(queue_summary.queue_name(), "email-delivery");
    assert_eq!(queue_summary.requeued(), 1);
    assert_eq!(queue_summary.dead_lettered(), 1);

    let retryable = sqlx::query_as::<_, (Vec<u8>, i16, String, i16, bool, bool)>(
        r#"
        SELECT
            payload,
            priority,
            state,
            delivery_attempts,
            receipt_handle IS NULL,
            visibility_deadline IS NULL
        FROM queue_message
        WHERE id = $1
        "#,
    )
    .bind(retryable_id)
    .fetch_one(&pool)
    .await?;

    assert_eq!(retryable.0, b"retryable-payload");
    assert_eq!(retryable.1, 3);
    assert_eq!(retryable.2, "READY");
    assert_eq!(retryable.3, 1);
    assert!(retryable.4, "requeued receipt handle should be cleared");
    assert!(
        retryable.5,
        "requeued visibility deadline should be cleared"
    );

    let exhausted_still_active: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM queue_message WHERE id = $1)")
            .bind(exhausted_id)
            .fetch_one(&pool)
            .await?;
    assert!(!exhausted_still_active);

    let dead_letter = sqlx::query_as::<_, (Uuid, Vec<u8>, i16, i16, String, bool)>(
        r#"
        SELECT
            queue_id,
            payload,
            priority,
            delivery_attempts,
            reason,
            expires_at IS NOT NULL
        FROM queue_dead_letter_message
        WHERE id = $1
        "#,
    )
    .bind(exhausted_id)
    .fetch_one(&pool)
    .await?;

    assert_eq!(dead_letter.0, queue_id);
    assert_eq!(dead_letter.1, b"exhausted-payload");
    assert_eq!(dead_letter.2, 2);
    assert_eq!(dead_letter.3, 2);
    assert_eq!(dead_letter.4, "MAX_DELIVERY_ATTEMPTS_EXHAUSTED");
    assert!(dead_letter.5, "expiration should be preserved");

    let state_after_processing = repository.queue_state(&mut lease).await?;
    let high_priority = state_for(&state_after_processing, MessagePriority::High);
    let medium_priority = state_for(&state_after_processing, MessagePriority::Medium);
    let low_priority = state_for(&state_after_processing, MessagePriority::Low);

    assert_eq!(high_priority.ready(), 1);
    assert_eq!(high_priority.in_flight(), 0);
    assert!(high_priority.oldest_ready_age_seconds() > 0.0);
    assert_eq!(high_priority.oldest_in_flight_age_seconds(), 0.0);

    for state in [medium_priority, low_priority] {
        assert_eq!(state.ready(), 0);
        assert_eq!(state.in_flight(), 0);
        assert_eq!(state.oldest_ready_age_seconds(), 0.0);
        assert_eq!(state.oldest_in_flight_age_seconds(), 0.0);
    }

    drop(lease);
    drop(repository);
    provider.shutdown().expect("provider should shut down");

    Ok(())
}

fn state_for(
    snapshot: &[QueuePriorityStateSnapshot],
    priority: MessagePriority,
) -> &QueuePriorityStateSnapshot {
    snapshot
        .iter()
        .find(|state| state.priority().as_str() == priority.as_str())
        .unwrap_or_else(|| panic!("missing {} priority state", priority.as_str()))
}
