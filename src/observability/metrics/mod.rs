mod cache;
mod database;
mod dead_letter_message_cleaner;
mod expired_message_cleaner;
mod http;
mod queue;
mod queue_commands;
mod queue_state;

use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkError,
    metrics::{Instrument, SdkMeterProvider, Stream},
};
use prometheus::{Encoder, Registry, TextEncoder};

pub(crate) use cache::CacheMetrics;
pub(crate) use database::DatabaseMetrics;
pub(crate) use dead_letter_message_cleaner::DeadLetterMessageCleanerMetrics;
pub(crate) use expired_message_cleaner::ExpiredMessageCleanerMetrics;
pub(crate) use http::HttpMetrics;
pub(crate) use queue::QueueInstrumentation;
pub(crate) use queue_commands::QueueCommandMetrics;
pub(crate) use queue_state::{QueuePriorityStateMetric, QueueStateMetrics};

#[derive(Clone)]
pub(crate) struct Metrics {
    registry: Registry,

    http: HttpMetrics,
    cache: CacheMetrics,
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

    pub(crate) fn cache(&self) -> &CacheMetrics {
        &self.cache
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
                | "queue.messages.dead_lettered"
                | "queue.dead_letter.messages.purged" => 1,

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
        cache: CacheMetrics::new(&meter),
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
#[path = "../../tests/observability_metrics.rs"]
mod tests;
