use std::time::Duration;

use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram, Meter},
};

#[derive(Clone)]
pub(crate) struct CacheMetrics {
    requests: Counter<u64>,
    load_duration: Histogram<f64>,
}

impl CacheMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let requests = meter
            .u64_counter("cache.requests")
            .with_description("Number of cache lookup requests")
            .with_unit("{request}")
            .build();

        let load_duration = meter
            .f64_histogram("cache.load.duration")
            .with_description("Time spent loading values after cache misses")
            .with_unit("s")
            .build();

        Self {
            requests,
            load_duration,
        }
    }

    pub(crate) fn request(&self, cache_name: &'static str, outcome: &'static str) {
        self.requests.add(
            1,
            &[
                KeyValue::new("cache.name", cache_name),
                KeyValue::new("outcome", outcome),
            ],
        );
    }

    pub(crate) fn load_finished(
        &self,
        cache_name: &'static str,
        duration: Duration,
        outcome: &'static str,
    ) {
        self.load_duration.record(
            duration.as_secs_f64(),
            &[
                KeyValue::new("cache.name", cache_name),
                KeyValue::new("outcome", outcome),
            ],
        );
    }
}
