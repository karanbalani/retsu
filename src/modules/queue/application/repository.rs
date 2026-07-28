use uuid::Uuid;

use super::super::domain::{Message, MessagePriority, Queue};

pub(in crate::modules::queue) enum CreateQueueOutcome {
    Created,
    AlreadyExists,
}

pub(in crate::modules::queue) enum EnqueueMessageOutcome {
    Enqueued,
    QueueNotFound,
}

pub(in crate::modules::queue) enum DequeueMessageOutcome {
    Dequeued {
        id: Uuid,
        payload: String,
        priority: MessagePriority,
        receipt_handle: Uuid,
        delivery_attempts: u16,
    },
    Empty,
    QueueNotFound,
}

pub(in crate::modules::queue) enum AcknowledgeMessageOutcome {
    Acknowledged,
    QueueNotFound,
    MessageNotFound,
    ReceiptHandleInvalid,
}

#[derive(Debug)]
pub(in crate::modules::queue) struct QueueTimeoutProcessingSummary {
    queue_name: String,
    requeued: u64,
    dead_lettered: u64,
}

impl QueueTimeoutProcessingSummary {
    pub fn new(queue_name: String, requeued: u64, dead_lettered: u64) -> Self {
        Self {
            queue_name,
            requeued,
            dead_lettered,
        }
    }

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn requeued(&self) -> u64 {
        self.requeued
    }

    pub(in crate::modules::queue) fn dead_lettered(&self) -> u64 {
        self.dead_lettered
    }

    pub(in crate::modules::queue) fn processed(&self) -> u64 {
        self.requeued + self.dead_lettered
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct TimeoutProcessingSummary {
    per_queue: Vec<QueueTimeoutProcessingSummary>,
}

impl TimeoutProcessingSummary {
    pub(in crate::modules::queue) fn new(per_queue: Vec<QueueTimeoutProcessingSummary>) -> Self {
        Self { per_queue }
    }

    pub(in crate::modules::queue) fn per_queue(&self) -> &[QueueTimeoutProcessingSummary] {
        &self.per_queue
    }

    pub(in crate::modules::queue) fn processed(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueTimeoutProcessingSummary::processed)
            .sum()
    }

    pub(in crate::modules::queue) fn requeued(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueTimeoutProcessingSummary::requeued)
            .sum()
    }

    pub(in crate::modules::queue) fn dead_lettered(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueTimeoutProcessingSummary::dead_lettered)
            .sum()
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct QueueExpiredMessagesCleanupSummary {
    queue_name: String,
    never_delivered: u64,
    previously_delivered: u64,
}

impl QueueExpiredMessagesCleanupSummary {
    pub fn new(queue_name: String, never_delivered: u64, previously_delivered: u64) -> Self {
        Self {
            queue_name,
            never_delivered,
            previously_delivered,
        }
    }

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn never_delivered(&self) -> u64 {
        self.never_delivered
    }

    pub(in crate::modules::queue) fn previously_delivered(&self) -> u64 {
        self.previously_delivered
    }

    pub(in crate::modules::queue) fn processed(&self) -> u64 {
        self.never_delivered + self.previously_delivered
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct ExpiredMessagesCleanupSummary {
    per_queue: Vec<QueueExpiredMessagesCleanupSummary>,
}

impl ExpiredMessagesCleanupSummary {
    pub(in crate::modules::queue) fn new(
        per_queue: Vec<QueueExpiredMessagesCleanupSummary>,
    ) -> Self {
        Self { per_queue }
    }

    pub(in crate::modules::queue) fn per_queue(&self) -> &[QueueExpiredMessagesCleanupSummary] {
        &self.per_queue
    }

    pub(in crate::modules::queue) fn processed(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueExpiredMessagesCleanupSummary::processed)
            .sum()
    }

    pub(in crate::modules::queue) fn never_delivered(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueExpiredMessagesCleanupSummary::never_delivered)
            .sum()
    }

    pub(in crate::modules::queue) fn previously_delivered(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueExpiredMessagesCleanupSummary::previously_delivered)
            .sum()
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct QueuePriorityStateSnapshot {
    queue_name: String,
    priority: MessagePriority,
    ready: u64,
    in_flight: u64,
    oldest_ready_age_seconds: f64,
    oldest_in_flight_age_seconds: f64,
}

impl QueuePriorityStateSnapshot {
    pub(in crate::modules::queue) fn new(
        queue_name: String,
        priority: MessagePriority,
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

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn priority(&self) -> MessagePriority {
        self.priority
    }

    pub(in crate::modules::queue) fn ready(&self) -> u64 {
        self.ready
    }

    pub(in crate::modules::queue) fn in_flight(&self) -> u64 {
        self.in_flight
    }

    pub(in crate::modules::queue) fn oldest_ready_age_seconds(&self) -> f64 {
        self.oldest_ready_age_seconds
    }

    pub(in crate::modules::queue) fn oldest_in_flight_age_seconds(&self) -> f64 {
        self.oldest_in_flight_age_seconds
    }
}

pub(in crate::modules::queue) trait QueueStateRepository {
    type CollectorLease;

    async fn try_acquire_collector_lease(
        &self,
    ) -> Result<Option<Self::CollectorLease>, anyhow::Error>;

    async fn queue_state(
        &self,
        lease: &mut Self::CollectorLease,
    ) -> Result<Vec<QueuePriorityStateSnapshot>, anyhow::Error>;
}

pub(in crate::modules::queue) trait QueueRepository {
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error>;

    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error>;
}

pub(in crate::modules::queue) trait MessageRepository {
    async fn enqueue_message(
        &self,
        queue_id: Uuid,
        message: &Message,
    ) -> Result<EnqueueMessageOutcome, anyhow::Error>;

    async fn dequeue_message(
        &self,
        queue_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error>;

    async fn acknowledge_message(
        &self,
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error>;

    async fn process_timed_out_messages(
        &self,
        batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, anyhow::Error>;

    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, anyhow::Error>;
}
