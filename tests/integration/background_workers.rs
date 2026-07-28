use std::time::Duration;

use super::harness::{IntegrationSystem, eventually, unique_queue_name};

#[tokio::test]
async fn visibility_timeout_worker_requeues_then_dead_letters() -> anyhow::Result<()> {
    let mut system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("visibility-timeout");

    let queue_id = system.create_queue(&queue_name, 1, 2, 60).await?;
    system.start_worker("visibility-timeout-processor").await?;

    let message_id = system
        .enqueue_message(queue_id, "retry-me", "HIGH", None)
        .await?;
    let first_delivery = system
        .dequeue_message(queue_id)
        .await?
        .expect("the enqueued message should be delivered");

    assert_eq!(first_delivery.id, message_id);
    assert_eq!(first_delivery.delivery_attempts, 1);

    let second_delivery = eventually(
        "the visibility-timeout worker to requeue the message",
        Duration::from_secs(20),
        || async {
            let message = system.dequeue_message(queue_id).await?;

            match message {
                Some(message) if message.id == message_id => Ok(Some(message)),
                Some(message) => anyhow::bail!("unexpected message {} was delivered", message.id),
                None => Ok(None),
            }
        },
    )
    .await?;

    assert_eq!(second_delivery.delivery_attempts, 2);
    assert_ne!(
        second_delivery.receipt_handle,
        first_delivery.receipt_handle
    );

    let rejection_code = system
        .rejected_acknowledgement_code(queue_id, message_id, first_delivery.receipt_handle)
        .await?;

    assert_eq!(rejection_code, "invalid_receipt_handle");
    assert!(system.message_exists(message_id).await?);

    let reason = eventually(
        "the exhausted message to be dead-lettered",
        Duration::from_secs(20),
        || async { system.dead_letter_reason(message_id).await },
    )
    .await?;

    assert_eq!(reason, "MAX_DELIVERY_ATTEMPTS_EXHAUSTED");
    assert!(!system.message_exists(message_id).await?);

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
