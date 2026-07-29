use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter},
};

#[derive(Clone)]
pub(crate) struct DeadLetterMessageCleanerMetrics {
    messages_purged: Counter<u64>,
}

impl DeadLetterMessageCleanerMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let messages_purged = meter
            .u64_counter("queue.dead_letter.messages.purged")
            .with_description("Number of messages purged from dead-letter storage")
            .with_unit("{message}")
            .build();

        Self { messages_purged }
    }

    pub(crate) fn messages_purged(&self, queue_name: &str, count: u64) {
        if count == 0 {
            return;
        }

        self.messages_purged
            .add(count, &[KeyValue::new("queue.name", queue_name.to_owned())]);
    }
}
