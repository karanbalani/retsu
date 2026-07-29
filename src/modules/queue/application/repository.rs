use uuid::Uuid;

use super::super::domain::{
    Message, MessagePriority, Queue, QueueConfigurationUpdate, QueueDetails,
};

pub(in crate::modules::queue) enum CreateQueueOutcome {
    Created,
    AlreadyExists,
}

pub(in crate::modules::queue) enum DequeueMessageOutcome {
    Dequeued {
        id: Uuid,
        payload: String,
        priority: MessagePriority,
        receipt_handle: Uuid,
        delivery_attempts: u16,
        queue_name: String,
        dead_lettered: u64,
    },
    Empty {
        queue_name: String,
        dead_lettered: u64,
    },
    QueueNotFound,
}

pub(in crate::modules::queue) enum AcknowledgeMessageOutcome {
    Acknowledged,
    Unchanged,
}

#[derive(Debug)]
pub(in crate::modules::queue) struct QueueDeadLetterMessagesPurgeSummary {
    queue_name: String,
    purged: u64,
}

impl QueueDeadLetterMessagesPurgeSummary {
    pub(in crate::modules::queue) fn new(queue_name: String, purged: u64) -> Self {
        Self { queue_name, purged }
    }

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn purged(&self) -> u64 {
        self.purged
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct DeadLetterMessagesPurgeSummary {
    per_queue: Vec<QueueDeadLetterMessagesPurgeSummary>,
}

impl DeadLetterMessagesPurgeSummary {
    pub(in crate::modules::queue) fn new(
        per_queue: Vec<QueueDeadLetterMessagesPurgeSummary>,
    ) -> Self {
        Self { per_queue }
    }

    pub(in crate::modules::queue) fn per_queue(&self) -> &[QueueDeadLetterMessagesPurgeSummary] {
        &self.per_queue
    }

    pub(in crate::modules::queue) fn purged(&self) -> u64 {
        self.per_queue
            .iter()
            .map(QueueDeadLetterMessagesPurgeSummary::purged)
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

pub(in crate::modules::queue) trait QueueRepository {
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error>;

    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error>;

    async fn queue_details(&self, queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error>;

    async fn update_queue(
        &self,
        queue_id: Uuid,
        configuration: &QueueConfigurationUpdate,
    ) -> Result<Option<QueueDetails>, anyhow::Error>;

    async fn enqueue_message(
        &self,
        queue_id: Uuid,
        message: &Message,
        effective_ttl_seconds: u32,
    ) -> Result<(), anyhow::Error>;

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

    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, anyhow::Error>;

    async fn purge_dead_letter_messages(
        &self,
        retention_seconds: i64,
        batch_size: u32,
    ) -> Result<DeadLetterMessagesPurgeSummary, anyhow::Error>;
}
