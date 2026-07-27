use std::time::Duration;

use opentelemetry::{
    KeyValue,
    metrics::{Histogram, Meter},
};
use sqlx::PgPool;

#[derive(Clone)]
pub(crate) struct DatabaseMetrics {
    meter: Meter,
    acquire_duration: Histogram<f64>,
    operation_duration: Histogram<f64>,
}

impl DatabaseMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let boundaries = vec![
            0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
        ];

        let acquire_duration = meter
            .f64_histogram("retsu.db.pool.acquire.duration")
            .with_description("Time spent acquiring a SQLx connection")
            .with_unit("s")
            .with_boundaries(boundaries.clone())
            .build();

        let operation_duration = meter
            .f64_histogram("retsu.db.operation.duration")
            .with_description("PostgreSQL query execution duration")
            .with_unit("s")
            .with_boundaries(boundaries)
            .build();

        Self {
            meter: meter.clone(),
            acquire_duration,
            operation_duration,
        }
    }

    pub(crate) fn acquisition_finished(&self, duration: Duration, succeeded: bool) {
        self.acquire_duration.record(
            duration.as_secs_f64(),
            &[KeyValue::new(
                "outcome",
                if succeeded { "success" } else { "error" },
            )],
        );
    }

    pub(crate) fn operation_finished(
        &self,
        operation: &'static str,
        duration: Duration,
        succeeded: bool,
    ) {
        self.operation_duration.record(
            duration.as_secs_f64(),
            &[
                KeyValue::new("db.operation.name", operation),
                KeyValue::new("outcome", if succeeded { "success" } else { "error" }),
            ],
        );
    }

    pub(crate) fn register_pool(&self, pool: PgPool, max_connections: u32) {
        let observed_pool = pool.clone();

        let _connections = self
            .meter
            .u64_observable_gauge("retsu.db.pool.connections")
            .with_description("Current SQLx connections by state")
            .with_callback(move |observer| {
                let size = u64::from(observed_pool.size());
                let idle = u64::try_from(observed_pool.num_idle())
                    .expect("pool idle count should fit u64");

                observer.observe(size.saturating_sub(idle), &[KeyValue::new("state", "used")]);

                observer.observe(idle, &[KeyValue::new("state", "idle")]);
            })
            .build();

        let _maximum = self
            .meter
            .u64_observable_gauge("retsu.db.pool.max_connections")
            .with_description("Configured SQLx connection limit")
            .with_callback(move |observer| {
                observer.observe(u64::from(max_connections), &[]);
            })
            .build();
    }
}
