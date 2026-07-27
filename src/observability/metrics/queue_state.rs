use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use opentelemetry::{
    KeyValue,
    metrics::{Counter, Gauge, Histogram, Meter},
};

#[derive(Debug)]
pub(crate) struct QueuePriorityStateMetric {
    queue_name: String,
    priority: &'static str,
    ready: u64,
    in_flight: u64,
    oldest_ready_age_seconds: f64,
    oldest_in_flight_age_seconds: f64,
}

impl QueuePriorityStateMetric {
    pub(crate) fn new(
        queue_name: String,
        priority: &'static str,
        ready: u64,
        in_flight: u64,
        oldest_ready_age_seconds: f64,
        oldest_in_flight_age_seconds: f64,
    ) -> Self {
        Self {
            queue_name,
            priority,
            ready,
            in_flight,
            oldest_ready_age_seconds,
            oldest_in_flight_age_seconds,
        }
    }

    fn attributes(&self) -> [KeyValue; 2] {
        [
            KeyValue::new("queue.name", self.queue_name.clone()),
            KeyValue::new("message.priority", self.priority),
        ]
    }
}

#[derive(Default)]
struct QueueStateMetricCache {
    queues: Vec<QueuePriorityStateMetric>,
    updated_at: Option<Instant>,
}

#[derive(Clone)]
pub(crate) struct QueueStateMetrics {
    state: Arc<RwLock<QueueStateMetricCache>>,
    collection_success: Gauge<u64>,
    collection_failures: Counter<u64>,
    collection_duration: Histogram<f64>,
}

impl QueueStateMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let state = Arc::new(RwLock::new(QueueStateMetricCache::default()));

        let ready_state = Arc::clone(&state);

        let _messages_ready = meter
            .u64_observable_gauge("queue.messages.ready")
            .with_description("Number of non-expired messages ready for delivery")
            .with_unit("{message}")
            .with_callback(move |observer| {
                let snapshot = ready_state
                    .read()
                    .expect("queue state metrics lock should not be poisoned");

                for queue in &snapshot.queues {
                    observer.observe(queue.ready, &queue.attributes());
                }
            })
            .build();

        let in_flight_state = Arc::clone(&state);

        let _messages_in_flight = meter
            .u64_observable_gauge("queue.messages.in_flight")
            .with_description("Number of messages with an active delivery lease")
            .with_unit("{message}")
            .with_callback(move |observer| {
                let snapshot = in_flight_state
                    .read()
                    .expect("queue state metrics lock should not be poisoned");

                for queue in &snapshot.queues {
                    observer.observe(queue.in_flight, &queue.attributes());
                }
            })
            .build();

        let oldest_ready_state = Arc::clone(&state);

        let _oldest_ready_message_age = meter
            .f64_observable_gauge("queue.oldest_ready_message.age")
            .with_description("Age of the oldest non-expired ready message")
            .with_unit("s")
            .with_callback(move |observer| {
                let snapshot = oldest_ready_state
                    .read()
                    .expect("queue state metrics lock should not be poisoned");

                for queue in &snapshot.queues {
                    observer.observe(queue.oldest_ready_age_seconds, &queue.attributes());
                }
            })
            .build();

        let oldest_in_flight_state = Arc::clone(&state);

        let _oldest_in_flight_message_age = meter
            .f64_observable_gauge("queue.oldest_in_flight_message.age")
            .with_description(
                "Age since enqueue of the oldest message with an active delivery lease",
            )
            .with_unit("s")
            .with_callback(move |observer| {
                let snapshot = oldest_in_flight_state
                    .read()
                    .expect("queue state metrics lock should not be poisoned");

                for queue in &snapshot.queues {
                    observer.observe(queue.oldest_in_flight_age_seconds, &queue.attributes());
                }
            })
            .build();

        let freshness_state = Arc::clone(&state);

        let _snapshot_age = meter
            .f64_observable_gauge("queue.state.snapshot.age")
            .with_description("Time since queue state metrics were last refreshed")
            .with_unit("s")
            .with_callback(move |observer| {
                let snapshot = freshness_state
                    .read()
                    .expect("queue state metrics lock should not be poisoned");

                if let Some(updated_at) = snapshot.updated_at.as_ref() {
                    observer.observe(updated_at.elapsed().as_secs_f64(), &[]);
                }
            })
            .build();

        let collection_success = meter
            .u64_gauge("queue.state.collection.success")
            .with_description("Whether the latest queue state collection succeeded")
            .build();

        let collection_failures = meter
            .u64_counter("queue.state.collection.failures")
            .with_description("Number of failed queue state collection attempts")
            .with_unit("{attempt}")
            .build();

        let collection_duration = meter
            .f64_histogram("queue.state.collection.duration")
            .with_description("Time spent collecting a queue state snapshot")
            .with_unit("s")
            .with_boundaries(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ])
            .build();

        Self {
            state,
            collection_success,
            collection_failures,
            collection_duration,
        }
    }

    pub(crate) fn replace(&self, queues: Vec<QueuePriorityStateMetric>) {
        let mut state = self
            .state
            .write()
            .expect("queue state metrics lock should not be poisoned");

        *state = QueueStateMetricCache {
            queues,
            updated_at: Some(Instant::now()),
        };
    }

    pub(crate) fn collection_finished(&self, duration: Duration, succeeded: bool) {
        let outcome = if succeeded { "success" } else { "error" };

        self.collection_duration
            .record(duration.as_secs_f64(), &[KeyValue::new("outcome", outcome)]);

        self.collection_success
            .record(if succeeded { 1 } else { 0 }, &[]);

        if !succeeded {
            self.collection_failures.add(1, &[]);
        }
    }
}
