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

pub(in crate::modules::queue) trait QueueRepository {
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error>;
}

pub(in crate::modules::queue) trait MessageRepository {
    async fn enqueue_message(
        &self,
        queue_name: &str,
        message: &Message,
    ) -> Result<EnqueueMessageOutcome, anyhow::Error>;

    async fn dequeue_message(
        &self,
        queue_name: &str,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error>;

    async fn acknowledge_message(
        &self,
        queue_name: &str,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error>;
}
