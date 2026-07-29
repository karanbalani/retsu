use std::time::Duration;

use super::harness::{
    DEQUEUE_PAUSE_LOCK_KEY, DEQUEUE_PAUSE_PAYLOAD, IntegrationSystem, eventually, unique_queue_name,
};

#[tokio::test]
async fn dequeue_retries_timed_out_messages_then_dead_letters_them() -> anyhow::Result<()> {
    let mut system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("visibility-timeout");

    let queue_id = system.create_queue(&queue_name, 1, 2, 60).await?;

    let message_id = system
        .enqueue_message(queue_id, "retry-me", "HIGH", None)
        .await?;
    let first_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("the enqueued message should be delivered");

    assert_eq!(first_delivery.id, message_id);
    assert_eq!(first_delivery.delivery_attempts, 1);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let second_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("the elapsed lease should make the message directly claimable");

    assert_eq!(second_delivery.delivery_attempts, 2);
    assert_ne!(
        second_delivery.receipt_handle,
        first_delivery.receipt_handle
    );

    system
        .acknowledge_message(queue_id, message_id, first_delivery.receipt_handle)
        .await?;
    assert!(system.message_exists(message_id).await?);

    tokio::time::sleep(Duration::from_secs(2)).await;

    assert!(system.dequeue_message(queue_id).await?.is_none());

    let reason = system
        .dead_letter_reason(message_id)
        .await?
        .expect("dequeue should move the exhausted message to dead-letter storage");

    assert_eq!(reason, "MAX_DELIVERY_ATTEMPTS_EXHAUSTED");
    assert!(!system.message_exists(message_id).await?);

    system.age_dead_letter(message_id, 1_209_601).await?;
    system.start_worker("dead-letter-message-cleaner").await?;

    eventually(
        "the retention worker to purge the dead-lettered message",
        Duration::from_secs(10),
        || async {
            let reason = system.dead_letter_reason(message_id).await?;
            Ok(reason.is_none().then_some(()))
        },
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn concurrent_dequeues_do_not_lock_an_unselected_state_candidate() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("mixed-state-concurrent-dequeue");

    let queue_id = system.create_queue(&queue_name, 1, 3, 60).await?;
    let retryable_id = system
        .enqueue_message(queue_id, DEQUEUE_PAUSE_PAYLOAD, "HIGH", None)
        .await?;

    let first_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("the retry candidate should receive its first lease");
    assert_eq!(first_delivery.id, retryable_id);

    let ready_id = system
        .enqueue_message(queue_id, "ready-during-retry", "LOW", None)
        .await?;

    tokio::time::sleep(Duration::from_secs(2)).await;
    system.install_dequeue_pause_trigger().await?;

    let mut pause_lock = system.hold_advisory_lock(DEQUEUE_PAUSE_LOCK_KEY).await?;

    let (retry, ready) = tokio::join!(system.dequeue_message(queue_id), async {
        system.wait_for_advisory_lock_waiter().await?;
        let delivery = system.dequeue_message(queue_id).await;
        system
            .release_advisory_lock(&mut pause_lock, DEQUEUE_PAUSE_LOCK_KEY)
            .await?;
        delivery
    },);

    let retry = retry?.expect("the elapsed high-priority lease should be retried");
    let ready = ready?.expect("the concurrent dequeue should claim the READY message");

    assert_eq!(retry.id, retryable_id);
    assert_eq!(ready.id, ready_id);

    Ok(())
}

#[tokio::test]
async fn exhausted_message_does_not_hide_a_deliverable_message() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("inline-dead-letter");

    let queue_id = system.create_queue(&queue_name, 1, 1, 60).await?;
    let exhausted_id = system
        .enqueue_message(queue_id, "exhausted", "HIGH", None)
        .await?;
    let deliverable_id = system
        .enqueue_message(queue_id, "deliverable", "LOW", None)
        .await?;

    let first_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("the high-priority message should be delivered first");
    assert_eq!(first_delivery.id, exhausted_id);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let next_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("dead-letter maintenance must not produce a false empty dequeue");

    assert_eq!(next_delivery.id, deliverable_id);
    assert_eq!(
        system.dead_letter_reason(exhausted_id).await?.as_deref(),
        Some("MAX_DELIVERY_ATTEMPTS_EXHAUSTED")
    );

    Ok(())
}

#[tokio::test]
async fn expired_message_worker_removes_stale_messages() -> anyhow::Result<()> {
    let mut system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("message-expiration");

    let queue_id = system.create_queue(&queue_name, 30, 3, 1).await?;

    let message_id = system
        .enqueue_message(queue_id, "expire-me", "MEDIUM", None)
        .await?;

    tokio::time::sleep(Duration::from_secs(2)).await;
    system.start_worker("expired-message-cleaner").await?;

    eventually(
        "the expired-message worker to remove the message",
        Duration::from_secs(10),
        || async {
            let exists = system.message_exists(message_id).await?;
            Ok((!exists).then_some(()))
        },
    )
    .await?;

    assert!(system.dead_letter_reason(message_id).await?.is_none());
    assert!(system.dequeue_message(queue_id).await?.is_none());

    Ok(())
}
