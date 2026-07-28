use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter},
};

#[derive(Clone)]
pub(crate) struct QueueCommandMetrics {
    messages_enqueued: Counter<u64>,
    messages_acknowledged: Counter<u64>,
    messages_dead_lettered: Counter<u64>,
}

impl QueueCommandMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let messages_enqueued = meter
            .u64_counter("queue.messages.enqueued")
            .with_description("Number of messages durably enqueued")
            .with_unit("{message}")
            .build();

        let messages_acknowledged = meter
            .u64_counter("queue.messages.acknowledged")
            .with_description("Number of messages successfully acknowledged")
            .with_unit("{message}")
            .build();

        let messages_dead_lettered = meter
            .u64_counter("queue.messages.dead_lettered")
            .with_description("Number of messages moved to dead-letter storage")
            .with_unit("{message}")
            .build();

        Self {
            messages_enqueued,
            messages_acknowledged,
            messages_dead_lettered,
        }
    }

    pub(crate) fn message_enqueued(&self, queue_name: &str, priority: &str) {
        self.messages_enqueued.add(
            1,
            &[
                KeyValue::new("queue.name", queue_name.to_owned()),
                KeyValue::new("message.priority", priority.to_owned()),
            ],
        );
    }

    pub(crate) fn message_acknowledged(&self, queue_name: &str) {
        self.messages_acknowledged
            .add(1, &[KeyValue::new("queue.name", queue_name.to_owned())]);
    }

    pub(crate) fn messages_dead_lettered(&self, queue_name: &str, count: u64) {
        if count == 0 {
            return;
        }

        self.messages_dead_lettered
            .add(count, &[KeyValue::new("queue.name", queue_name.to_owned())]);
    }
}
