use super::harness::{IntegrationSystem, unique_queue_name};

#[tokio::test]
async fn queue_lifecycle_crosses_real_process_and_database_boundaries() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("queue-lifecycle");

    system.create_queue(&queue_name, 30, 3, 300).await?;

    let low_id = system
        .enqueue_message(&queue_name, "low-priority", "LOW", None)
        .await?;
    let first_high_id = system
        .enqueue_message(&queue_name, "first-high-priority", "HIGH", None)
        .await?;
    let second_high_id = system
        .enqueue_message(&queue_name, "second-high-priority", "HIGH", None)
        .await?;

    let first_high = system
        .dequeue_message(&queue_name)
        .await?
        .expect("the high-priority message should be available");

    assert_eq!(first_high.id, first_high_id);
    assert_eq!(first_high.payload, "first-high-priority");
    assert_eq!(first_high.priority, "HIGH");
    assert_eq!(first_high.delivery_attempts, 1);

    system
        .acknowledge_message(&queue_name, first_high.id, first_high.receipt_handle)
        .await?;

    let second_high = system
        .dequeue_message(&queue_name)
        .await?
        .expect("the second high-priority message should be available");

    assert_eq!(second_high.id, second_high_id);
    assert_eq!(second_high.payload, "second-high-priority");
    assert_eq!(second_high.priority, "HIGH");
    assert_eq!(second_high.delivery_attempts, 1);

    system
        .acknowledge_message(&queue_name, second_high.id, second_high.receipt_handle)
        .await?;

    let low = system
        .dequeue_message(&queue_name)
        .await?
        .expect("the low-priority message should remain available");

    assert_eq!(low.id, low_id);
    assert_eq!(low.payload, "low-priority");
    assert_eq!(low.priority, "LOW");
    assert_eq!(low.delivery_attempts, 1);

    system
        .acknowledge_message(&queue_name, low.id, low.receipt_handle)
        .await?;

    assert!(system.dequeue_message(&queue_name).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn concurrent_dequeues_lease_a_message_only_once() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("concurrent-dequeue");

    system.create_queue(&queue_name, 30, 3, 300).await?;
    let message_id = system
        .enqueue_message(&queue_name, "lease-once", "MEDIUM", None)
        .await?;

    let (first, second, third, fourth) = tokio::join!(
        system.dequeue_message(&queue_name),
        system.dequeue_message(&queue_name),
        system.dequeue_message(&queue_name),
        system.dequeue_message(&queue_name),
    );

    let mut deliveries = [first?, second?, third?, fourth?]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    assert_eq!(
        deliveries.len(),
        1,
        "concurrent consumers must not lease the same message more than once"
    );

    let delivery = deliveries.pop().expect("one consumer should win the lease");
    assert_eq!(delivery.id, message_id);
    assert_eq!(delivery.delivery_attempts, 1);

    system
        .acknowledge_message(&queue_name, delivery.id, delivery.receipt_handle)
        .await?;
    assert!(!system.message_exists(message_id).await?);

    Ok(())
}
