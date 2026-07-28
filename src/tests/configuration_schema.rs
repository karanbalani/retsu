use validator::Validate as _;

use super::AppConfiguration;

fn assert_invalid(mutate: impl FnOnce(&mut AppConfiguration)) {
    let mut configuration = AppConfiguration::default();
    mutate(&mut configuration);

    assert!(
        configuration.validate().is_err(),
        "configuration should be rejected"
    );
}

#[test]
fn accepts_all_validation_boundaries() {
    let mut configuration = AppConfiguration::default();
    configuration.http.port = 1;
    configuration.cache.queue_details.max_entries = 1;
    configuration.cache.queue_details.max_capacity_bytes = 1;
    configuration.cache.queue_details.ttl_seconds = 1;
    configuration.telemetry.metrics.max_queues = 1;
    configuration.telemetry.traces.timeout_seconds = 1;
    configuration.database.max_connections = 1;
    configuration.database.acquire_timeout_seconds = 5;
    configuration.worker.shutdown_timeout_seconds = 1;
    configuration.worker.management.port = 1;

    configuration
        .validate()
        .expect("minimum boundaries should be valid");

    configuration.telemetry.metrics.max_queues = 100_000;
    configuration.telemetry.traces.timeout_seconds = 60;
    configuration.cache.queue_details.max_entries = 1_000_000;
    configuration.cache.queue_details.max_capacity_bytes = 4_294_967_295;
    configuration.cache.queue_details.ttl_seconds = 86_400;
    configuration.database.acquire_timeout_seconds = 60;
    configuration.worker.shutdown_timeout_seconds = 300;

    configuration
        .validate()
        .expect("maximum boundaries should be valid");
}

#[test]
fn rejects_values_outside_the_validation_contract() {
    assert_invalid(|configuration| configuration.http.port = 0);
    assert_invalid(|configuration| configuration.logging.filter.clear());
    assert_invalid(|configuration| configuration.cache.queue_details.max_entries = 0);
    assert_invalid(|configuration| configuration.cache.queue_details.max_entries = 1_000_001);
    assert_invalid(|configuration| configuration.cache.queue_details.max_capacity_bytes = 0);
    assert_invalid(|configuration| {
        configuration.cache.queue_details.max_capacity_bytes = 4_294_967_296;
    });
    assert_invalid(|configuration| configuration.cache.queue_details.ttl_seconds = 0);
    assert_invalid(|configuration| configuration.cache.queue_details.ttl_seconds = 86_401);
    assert_invalid(|configuration| configuration.telemetry.metrics.max_queues = 0);
    assert_invalid(|configuration| configuration.telemetry.metrics.max_queues = 100_001);
    assert_invalid(|configuration| configuration.telemetry.traces.filter.clear());
    assert_invalid(|configuration| {
        configuration.telemetry.traces.endpoint = "not a URL".to_owned();
    });
    assert_invalid(|configuration| configuration.telemetry.traces.timeout_seconds = 0);
    assert_invalid(|configuration| configuration.telemetry.traces.timeout_seconds = 61);
    assert_invalid(|configuration| {
        configuration.database.url = "not a URL".to_owned();
    });
    assert_invalid(|configuration| configuration.database.max_connections = 0);
    assert_invalid(|configuration| configuration.database.acquire_timeout_seconds = 4);
    assert_invalid(|configuration| configuration.database.acquire_timeout_seconds = 61);
    assert_invalid(|configuration| configuration.worker.shutdown_timeout_seconds = 0);
    assert_invalid(|configuration| configuration.worker.shutdown_timeout_seconds = 301);
    assert_invalid(|configuration| configuration.worker.management.port = 0);
}
