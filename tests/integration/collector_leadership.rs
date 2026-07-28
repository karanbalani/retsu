use std::time::Duration;

use super::harness::{IntegrationSystem, eventually, unique_queue_name};

#[tokio::test]
async fn state_collector_leadership_fails_over_between_processes() -> anyhow::Result<()> {
    let mut system = IntegrationSystem::start().await?;
    let queue_name = unique_queue_name("collector-leadership");

    let queue_id = system.create_queue(&queue_name, 1, 3, 300).await?;

    let timed_out_id = system
        .enqueue_message(queue_id, "timed-out", "HIGH", None)
        .await?;
    let timed_out = system
        .dequeue_message(queue_id)
        .await?
        .expect("the first message should be delivered");
    assert_eq!(timed_out.id, timed_out_id);

    system
        .enqueue_message(queue_id, "ready", "HIGH", None)
        .await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let leader = system.start_worker("state-metrics-collector").await?;

    let leader_metrics = eventually(
        "the first collector to publish a queue-state snapshot",
        Duration::from_secs(5),
        || async {
            let metrics = system.worker_metrics(&leader).await?;

            if has_successful_snapshot(&metrics) {
                Ok(Some(metrics))
            } else {
                anyhow::bail!("collector metrics did not contain success:\n{metrics}")
            }
        },
    )
    .await?;

    assert_queue_state(&leader_metrics, &queue_name);

    let standby = system.start_worker("state-metrics-collector").await?;
    let standby_metrics = system.worker_metrics(&standby).await?;

    assert!(
        !has_successful_snapshot(&standby_metrics),
        "the standby collector must not publish a snapshot while leadership is held"
    );

    system.stop_worker(&leader)?;

    let standby_metrics = eventually(
        "the standby collector to acquire leadership",
        Duration::from_secs(30),
        || async {
            let metrics = system.worker_metrics(&standby).await?;

            if has_successful_snapshot(&metrics) {
                Ok(Some(metrics))
            } else {
                anyhow::bail!("standby metrics did not contain success:\n{metrics}")
            }
        },
    )
    .await?;

    assert_queue_state(&standby_metrics, &queue_name);

    Ok(())
}

fn has_successful_snapshot(metrics: &str) -> bool {
    metrics
        .lines()
        .any(|line| line.starts_with("queue_state_collection_success{") && line.ends_with("} 1"))
}

fn assert_queue_state(metrics: &str, queue_name: &str) {
    assert!(
        has_queue_metric(metrics, "queue_messages_ready", queue_name, "2"),
        "the ready message and elapsed lease should appear as logically ready"
    );
    assert!(
        has_queue_metric(metrics, "queue_messages_in_flight", queue_name, "0"),
        "the timed-out delivery should not appear as logically in flight"
    );
}

fn has_queue_metric(metrics: &str, metric: &str, queue_name: &str, value: &str) -> bool {
    metrics.lines().any(|line| {
        line.starts_with(&format!("{metric}{{"))
            && line.contains("message_priority=\"HIGH\"")
            && line.contains(&format!("queue_name=\"{queue_name}\""))
            && line.ends_with(&format!("}} {value}"))
    })
}
