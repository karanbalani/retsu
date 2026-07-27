use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter},
};

#[derive(Clone)]
pub(crate) struct VisibilityTimeoutMetrics {
    messages_requeued: Counter<u64>,
    messages_dead_lettered: Counter<u64>,
}

impl VisibilityTimeoutMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let messages_requeued = meter
            .u64_counter("queue.messages.requeued")
            .with_description("Number of messages requeued after a visibility timeout")
            .with_unit("{message}")
            .build();

        let messages_dead_lettered = meter
            .u64_counter("queue.messages.dead_lettered")
            .with_description("Number of messages moved to dead-letter storage")
            .with_unit("{message}")
            .build();

        Self {
            messages_requeued,
            messages_dead_lettered,
        }
    }

    pub(crate) fn messages_requeued(&self, queue_name: &str, count: u64) {
        if count == 0 {
            return;
        }

        self.messages_requeued
            .add(count, &[KeyValue::new("queue.name", queue_name.to_owned())]);
    }

    pub(crate) fn messages_dead_lettered(&self, queue_name: &str, count: u64) {
        if count == 0 {
            return;
        }

        self.messages_dead_lettered
            .add(count, &[KeyValue::new("queue.name", queue_name.to_owned())]);
    }
}
