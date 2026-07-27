use std::time::Duration;

use super::test_metrics;

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
