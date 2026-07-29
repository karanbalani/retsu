use super::harness::{IntegrationSystem, unique_queue_name};

#[tokio::test]
async fn queue_details_cache_handles_write_read_and_outage_paths() -> anyhow::Result<()> {
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

    let write_through_default_id = system
        .enqueue_message(created_id, "write-through-default", "MEDIUM", None)
        .await?;
    assert_eq!(
        system.message_ttl_seconds(write_through_default_id).await?,
        900.0
    );

    let explicit_ttl_id = system
        .enqueue_message(created_id, "explicit-ttl", "MEDIUM", Some(30))
        .await?;
    assert_eq!(system.message_ttl_seconds(explicit_ttl_id).await?, 30.0);

    let inserted_name = unique_queue_name("cache-read-through");
    let inserted_id = system.insert_queue(&inserted_name, 60, 9, 600).await?;
    assert!(
        system
            .distributed_queue_details(inserted_id)
            .await?
            .is_none(),
        "direct database insert should start outside the cache"
    );

    let read_through_default_id = system
        .enqueue_message(inserted_id, "populate-cache", "MEDIUM", None)
        .await?;
    assert_eq!(
        system.message_ttl_seconds(read_through_default_id).await?,
        600.0
    );

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

    system.stop_distributed_cache().await?;

    let fallback_name = unique_queue_name("cache-outage-fallback");
    let fallback_id = system.insert_queue(&fallback_name, 75, 5, 450).await?;
    let fallback_message_id = system
        .enqueue_message(fallback_id, "postgres-fallback", "MEDIUM", None)
        .await?;

    assert!(system.message_exists(fallback_message_id).await?);

    Ok(())
}
