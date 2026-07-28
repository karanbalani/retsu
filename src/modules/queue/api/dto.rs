use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::application::{
    AcknowledgeMessageCommand, CreateQueueCommand, DequeueMessageCommand, DequeuedMessage,
    EnqueueMessageCommand, EnqueuedMessage, UpdateQueueCommand,
};
use super::super::domain::QueueDetails;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateQueueRequest {
    name: String,
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
    default_message_ttl_seconds: Option<u32>,
}

impl CreateQueueRequest {
    pub(super) fn into_command(self) -> CreateQueueCommand {
        CreateQueueCommand::new(
            self.name,
            self.visibility_timeout_seconds,
            self.max_delivery_attempts,
            self.default_message_ttl_seconds,
        )
    }
}

#[derive(Debug, Serialize)]
pub(super) struct QueueResponse {
    id: Uuid,
    name: String,
    visibility_timeout_seconds: u32,
    max_delivery_attempts: u16,
    default_message_ttl_seconds: u32,
}

impl From<QueueDetails> for QueueResponse {
    fn from(queue: QueueDetails) -> Self {
        Self {
            id: queue.id(),
            name: queue.name().to_owned(),
            visibility_timeout_seconds: queue.visibility_timeout_seconds(),
            max_delivery_attempts: queue.max_delivery_attempts(),
            default_message_ttl_seconds: queue.default_message_ttl_seconds(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct QueuePath {
    queue_id: Uuid,
}

impl QueuePath {
    pub(super) fn into_queue_id(self) -> Uuid {
        self.queue_id
    }

    pub(super) fn into_dequeue_command(self) -> DequeueMessageCommand {
        DequeueMessageCommand::new(self.queue_id)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateQueueRequest {
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
    default_message_ttl_seconds: Option<u32>,
}

impl UpdateQueueRequest {
    pub(super) fn into_command(self, queue_id: Uuid) -> UpdateQueueCommand {
        UpdateQueueCommand::new(
            queue_id,
            self.visibility_timeout_seconds,
            self.max_delivery_attempts,
            self.default_message_ttl_seconds,
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnqueueMessageRequest {
    payload: String,
    priority: String,
    ttl_seconds: Option<u32>,
}

impl EnqueueMessageRequest {
    pub(super) fn into_command(self, queue_id: Uuid) -> EnqueueMessageCommand {
        EnqueueMessageCommand::new(queue_id, self.payload, self.priority, self.ttl_seconds)
    }
}

#[derive(Debug, Serialize)]
pub(super) struct EnqueueMessageResponse {
    id: Uuid,
}

impl From<EnqueuedMessage> for EnqueueMessageResponse {
    fn from(message: EnqueuedMessage) -> Self {
        Self { id: message.id() }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct DequeueMessageResponse {
    id: Uuid,
    payload: String,
    priority: &'static str,
    receipt_handle: Uuid,
    delivery_attempts: u16,
}

impl From<DequeuedMessage> for DequeueMessageResponse {
    fn from(message: DequeuedMessage) -> Self {
        Self {
            id: message.id(),
            payload: message.payload().to_owned(),
            priority: message.priority(),
            receipt_handle: message.receipt_handle(),
            delivery_attempts: message.delivery_attempts(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct MessagePath {
    queue_id: Uuid,
    message_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AcknowledgeMessageRequest {
    receipt_handle: Uuid,
}

impl AcknowledgeMessageRequest {
    pub(super) fn into_command(self, path: MessagePath) -> AcknowledgeMessageCommand {
        AcknowledgeMessageCommand::new(path.queue_id, path.message_id, self.receipt_handle)
    }
}
