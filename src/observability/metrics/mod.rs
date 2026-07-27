mod database;
mod expired_message_cleaner;
mod http;
mod queue;
mod queue_commands;
mod queue_state;
mod visibility_timeout;

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkError,
    metrics::{Instrument, SdkMeterProvider, Stream},
};
use prometheus::{Encoder, Registry, TextEncoder};

pub(crate) use database::DatabaseMetrics;
pub(crate) use expired_message_cleaner::ExpiredMessageCleanerMetrics;
pub(crate) use http::HttpMetrics;
pub(crate) use queue::QueueInstrumentation;
pub(crate) use queue_commands::QueueCommandMetrics;
pub(crate) use queue_state::{QueuePriorityStateMetric, QueueStateMetrics};
pub(crate) use visibility_timeout::VisibilityTimeoutMetrics;

#[derive(Clone)]
pub(crate) struct Metrics {
    registry: Registry,

    http: HttpMetrics,
    database: DatabaseMetrics,
    queue: QueueInstrumentation,
}

impl Metrics {
    pub(crate) fn http(&self) -> &HttpMetrics {
        &self.http
    }

    pub(crate) fn database(&self) -> &DatabaseMetrics {
        &self.database
    }

    pub(crate) fn queue(&self) -> &QueueInstrumentation {
        &self.queue
    }

    pub(crate) fn encode_prometheus(&self) -> Result<Vec<u8>, prometheus::Error> {
        let metric_families = self.registry.gather();
        let mut body = Vec::new();

        TextEncoder::new().encode(&metric_families, &mut body)?;

        Ok(body)
    }
}

pub(super) fn initialize(
    resource: Resource,
    max_queues: u32,
) -> Result<(SdkMeterProvider, Metrics), OTelSdkError> {
    let registry = Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()?;

    let max_queues = usize::try_from(max_queues).expect("u32 queue limit should fit into usize");

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(exporter)
        .with_view(move |instrument: &Instrument| {
            let multiplier = match instrument.name() {
                "queue.messages.enqueued"
                | "queue.messages.ready"
                | "queue.messages.in_flight"
                | "queue.oldest_ready_message.age"
                | "queue.oldest_in_flight_message.age" => 3,

                "queue.messages.expired" => 2,

                "queue.messages.acknowledged"
                | "queue.messages.requeued"
                | "queue.messages.dead_lettered" => 1,

                _ => return None,
            };

            let cardinality_limit = max_queues
                .checked_mul(multiplier)
                .expect("validated queue metric cardinality should fit into usize");

            Some(
                Stream::builder()
                    .with_cardinality_limit(cardinality_limit)
                    .build()
                    .expect("validated queue metric stream should build"),
            )
        })
        .build();

    let meter = provider.meter(env!("CARGO_PKG_NAME"));

    let metrics = Metrics {
        registry,

        http: HttpMetrics::new(&meter),
        database: DatabaseMetrics::new(&meter),
        queue: QueueInstrumentation::new(&meter),
    };

    Ok((provider, metrics))
}

#[cfg(test)]
pub(crate) fn test_metrics() -> (SdkMeterProvider, Metrics) {
    initialize(Resource::builder_empty().build(), 10_000).expect("test metrics should initialize")
}

#[cfg(test)]
mod tests {
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
        let success_count =
            sample_line(&output, "http_server_request_duration_seconds_count", "200");
        let success_sum = sample_line(&output, "http_server_request_duration_seconds_sum", "200");
        let failure_count =
            sample_line(&output, "http_server_request_duration_seconds_count", "503");

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
}
