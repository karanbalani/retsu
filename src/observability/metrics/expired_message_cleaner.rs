use opentelemetry::{
    KeyValue,
    metrics::{Counter, Meter},
};

#[derive(Clone)]
pub(crate) struct ExpiredMessageCleanerMetrics {
    messages_expired: Counter<u64>,
}

impl ExpiredMessageCleanerMetrics {
    pub(super) fn new(meter: &Meter) -> Self {
        let messages_expired = meter
            .u64_counter("queue.messages.expired")
            .with_description("Number of expired messages removed from active queue storage")
            .with_unit("{message}")
            .build();

        Self { messages_expired }
    }

    pub(crate) fn messages_expired(
        &self,
        queue_name: &str,
        delivery_history: &'static str,
        count: u64,
    ) {
        if count == 0 {
            return;
        }

        self.messages_expired.add(
            count,
            &[
                KeyValue::new("queue.name", queue_name.to_owned()),
                KeyValue::new("message.delivery_history", delivery_history),
            ],
        );
    }
}
