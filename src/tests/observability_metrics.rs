use std::time::Duration;

use super::{QueuePriorityStateMetric, test_metrics};

fn sample_line<'a>(output: &'a str, metric: &str, status_code: &str) -> &'a str {
    output
        .lines()
        .find(|line| {
            line.starts_with(metric)
                && line.contains(&format!("http_response_status_code=\"{status_code}\""))
        })
        .unwrap_or_else(|| panic!("missing metric `{metric}` for status {status_code}"))
}

#[test]
fn active_request_guard_increments_and_decrements_the_gauge() {
    let (provider, metrics) = test_metrics();

    let active_request = metrics
        .http()
        .request_started("GET".to_owned(), "https".to_owned());
    let active_output =
        String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
            .expect("metrics should be UTF-8");
    let active_line = active_output
        .lines()
        .find(|line| {
            line.starts_with("http_server_active_requests")
                && line.contains("http_request_method=\"GET\"")
                && line.contains("url_scheme=\"https\"")
        })
        .expect("active request metric should exist");

    assert!(active_line.ends_with(" 1"));

    drop(active_request);

    let inactive_output =
        String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
            .expect("metrics should be UTF-8");
    let inactive_line = inactive_output
        .lines()
        .find(|line| {
            line.starts_with("http_server_active_requests")
                && line.contains("http_request_method=\"GET\"")
                && line.contains("url_scheme=\"https\"")
        })
        .expect("inactive request metric should exist");

    assert!(inactive_line.ends_with(" 0"));

    provider.shutdown().expect("provider should shut down");
}

#[test]
fn request_duration_uses_bounded_semantic_attributes() {
    let (provider, metrics) = test_metrics();

    metrics.http().request_finished(
        "GET".to_owned(),
        "https".to_owned(),
        Some("1.1"),
        Some("/items/{identifier}".to_owned()),
        200,
        Duration::from_millis(250),
    );
    metrics.http().request_finished(
        "POST".to_owned(),
        "http".to_owned(),
        None,
        None,
        503,
        Duration::from_millis(500),
    );

    let output = String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
        .expect("metrics should be UTF-8");
    let success_count = sample_line(&output, "http_server_request_duration_seconds_count", "200");
    let success_sum = sample_line(&output, "http_server_request_duration_seconds_sum", "200");
    let failure_count = sample_line(&output, "http_server_request_duration_seconds_count", "503");

    assert!(success_count.contains("http_request_method=\"GET\""));
    assert!(success_count.contains("url_scheme=\"https\""));
    assert!(success_count.contains("network_protocol_version=\"1.1\""));
    assert!(success_count.contains("http_route=\"/items/{identifier}\""));
    assert!(!success_count.contains("error_type"));
    assert!(success_count.ends_with(" 1"));
    assert!(success_sum.ends_with(" 0.25"));

    assert!(failure_count.contains("http_request_method=\"POST\""));
    assert!(failure_count.contains("url_scheme=\"http\""));
    assert!(failure_count.contains("error_type=\"503\""));
    assert!(!failure_count.contains("network_protocol_version"));
    assert!(!failure_count.contains("http_route"));
    assert!(failure_count.ends_with(" 1"));

    provider.shutdown().expect("provider should shut down");
}

#[test]
fn cache_metrics_report_named_hits_misses_and_load_outcomes() {
    let (provider, metrics) = test_metrics();
    let cache = metrics.cache();

    cache.request("queue_details", "hit");
    cache.request("queue_details", "miss");
    cache.load_finished("queue_details", Duration::from_millis(25), "success");

    let output = String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
        .expect("metrics should be UTF-8");

    assert_metric_value(
        &output,
        "cache_requests_total",
        &[("cache_name", "queue_details"), ("outcome", "hit")],
        "1",
    );
    assert_metric_value(
        &output,
        "cache_requests_total",
        &[("cache_name", "queue_details"), ("outcome", "miss")],
        "1",
    );
    assert_metric_value(
        &output,
        "cache_load_duration_seconds_count",
        &[("cache_name", "queue_details"), ("outcome", "success")],
        "1",
    );

    provider.shutdown().expect("provider should shut down");
}

#[test]
fn queue_command_metrics_use_queue_names() {
    let (provider, metrics) = test_metrics();
    let commands = metrics.queue().commands();

    commands.message_enqueued("email-delivery", "HIGH");
    commands.message_acknowledged("email-delivery");

    let output = String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
        .expect("metrics should be UTF-8");

    assert_metric_value(
        &output,
        "queue_messages_enqueued_total",
        &[
            ("queue_name", "email-delivery"),
            ("message_priority", "HIGH"),
        ],
        "1",
    );
    assert_metric_value(
        &output,
        "queue_messages_acknowledged_total",
        &[("queue_name", "email-delivery")],
        "1",
    );
    assert!(
        !output.contains("queue_id="),
        "queue command metrics should not expose queue IDs"
    );

    provider.shutdown().expect("provider should shut down");
}

#[test]
fn queue_state_metrics_replace_stale_series_and_report_collection_health() {
    let (provider, metrics) = test_metrics();
    let state = metrics.queue().state();

    state.replace(vec![QueuePriorityStateMetric::new(
        "email-delivery".to_owned(),
        "HIGH",
        7,
        2,
        30.0,
        10.0,
    )]);
    state.collection_finished(Duration::from_millis(25), true);

    let first_output =
        String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
            .expect("metrics should be UTF-8");

    let email_high = [
        ("queue_name", "email-delivery"),
        ("message_priority", "HIGH"),
    ];
    for (metric, value) in [
        ("queue_messages_ready", "7"),
        ("queue_messages_in_flight", "2"),
        ("queue_oldest_ready_message_age_seconds", "30"),
        ("queue_oldest_in_flight_message_age_seconds", "10"),
    ] {
        assert_metric_value(&first_output, metric, &email_high, value);
    }
    assert!(first_output.contains("queue_state_snapshot_age_seconds"));
    assert_metric_value(&first_output, "queue_state_collection_success", &[], "1");

    state.replace(vec![QueuePriorityStateMetric::new(
        "sms-delivery".to_owned(),
        "LOW",
        3,
        1,
        8.0,
        4.0,
    )]);
    state.collection_finished(Duration::from_millis(20), true);
    state.collection_finished(Duration::from_millis(40), false);

    let second_output =
        String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
            .expect("metrics should be UTF-8");

    assert!(
        !second_output.contains("email-delivery"),
        "replaced snapshots should not retain stale queue series"
    );
    assert_metric_value(
        &second_output,
        "queue_messages_ready",
        &[("queue_name", "sms-delivery"), ("message_priority", "LOW")],
        "3",
    );
    assert_metric_value(&second_output, "queue_state_collection_success", &[], "0");
    assert_metric_value(
        &second_output,
        "queue_state_collection_failures_total",
        &[],
        "1",
    );
    assert_metric_value(
        &second_output,
        "queue_state_collection_duration_seconds_count",
        &[("outcome", "success")],
        "2",
    );
    assert_metric_value(
        &second_output,
        "queue_state_collection_duration_seconds_count",
        &[("outcome", "error")],
        "1",
    );

    provider.shutdown().expect("provider should shut down");
}

fn assert_metric_value(output: &str, metric: &str, labels: &[(&str, &str)], value: &str) {
    let line = output
        .lines()
        .find(|line| {
            line.starts_with(metric)
                && labels
                    .iter()
                    .all(|(name, value)| line.contains(&format!("{name}=\"{value}\"")))
        })
        .unwrap_or_else(|| panic!("missing metric `{metric}` with labels {labels:?}"));

    assert!(
        line.ends_with(&format!(" {value}")),
        "unexpected metric line: {line}"
    );
}
