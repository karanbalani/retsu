use super::harness::{IntegrationSystem, unique_queue_name};

#[tokio::test]
async fn queue_details_are_written_through_and_read_through_dragonfly() -> anyhow::Result<()> {
    let system = IntegrationSystem::start().await?;

    let created_name = unique_queue_name("cache-write-through");
    let created_id = system.create_queue(&created_name, 45, 7, 300).await?;
    let created_details = system
        .distributed_queue_details(created_id)
        .await?
        .expect("created queue details should be written through");

    assert_eq!(created_details["found"]["id"], created_id.to_string());
    assert_eq!(created_details["found"]["name"], created_name);
    assert_eq!(created_details["found"]["visibility_timeout_seconds"], 45);
    assert_eq!(created_details["found"]["max_delivery_attempts"], 7);
    assert_eq!(created_details["found"]["default_message_ttl_seconds"], 300);

    let updated = system
        .update_queue(created_id, Some(90), None, Some(900))
        .await?;
    assert_eq!(updated.id, created_id);
    assert_eq!(updated.name, created_name);
    assert_eq!(updated.visibility_timeout_seconds, 90);
    assert_eq!(updated.max_delivery_attempts, 7);
    assert_eq!(updated.default_message_ttl_seconds, 900);

    let updated_details = system
        .distributed_queue_details(created_id)
        .await?
        .expect("updated queue details should be written through");
    assert_eq!(updated_details["found"]["visibility_timeout_seconds"], 90);
    assert_eq!(updated_details["found"]["max_delivery_attempts"], 7);
    assert_eq!(updated_details["found"]["default_message_ttl_seconds"], 900);

    let inserted_name = unique_queue_name("cache-read-through");
    let inserted_id = system.insert_queue(&inserted_name, 60, 9, 600).await?;
    assert!(
        system
            .distributed_queue_details(inserted_id)
            .await?
            .is_none(),
        "direct database insert should start outside the cache"
    );

    system
        .enqueue_message(inserted_id, "populate-cache", "MEDIUM", None)
        .await?;

    let inserted_details = system
        .distributed_queue_details(inserted_id)
        .await?
        .expect("enqueue queue lookup should populate distributed details");

    assert_eq!(inserted_details["found"]["id"], inserted_id.to_string());
    assert_eq!(inserted_details["found"]["name"], inserted_name);
    assert_eq!(inserted_details["found"]["visibility_timeout_seconds"], 60);
    assert_eq!(inserted_details["found"]["max_delivery_attempts"], 9);
    assert_eq!(
        inserted_details["found"]["default_message_ttl_seconds"],
        600
    );

    Ok(())
}
