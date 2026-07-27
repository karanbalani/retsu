use super::harness::{IntegrationSystem, unique_queue_name};

#[tokio::test]
async fn queue_lifecycle_crosses_real_process_and_database_boundaries() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("queue-lifecycle");

    system.create_queue(&queue_name, 30, 3, 300).await?;

    let low_id = system
        .enqueue_message(&queue_name, "low-priority", "LOW", None)
        .await?;
    let high_id = system
        .enqueue_message(&queue_name, "high-priority", "HIGH", None)
        .await?;

    let high = system
        .dequeue_message(&queue_name)
        .await?
        .expect("the high-priority message should be available");

    assert_eq!(high.id, high_id);
    assert_eq!(high.payload, "high-priority");
    assert_eq!(high.priority, "HIGH");
    assert_eq!(high.delivery_attempts, 1);

    system
        .acknowledge_message(&queue_name, high.id, high.receipt_handle)
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
