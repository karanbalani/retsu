use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::application::{
    CreateQueueCommand, CreatedQueue, EnqueueMessageCommand, EnqueuedMessage,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateQueueRequest {
    name: String,
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
}

impl CreateQueueRequest {
    pub(super) fn into_command(self) -> CreateQueueCommand {
        CreateQueueCommand::new(
            self.name,
            self.visibility_timeout_seconds,
            self.max_delivery_attempts,
        )
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CreateQueueResponse {
    id: Uuid,
    name: String,
    visibility_timeout_seconds: u32,
    max_delivery_attempts: u16,
}

impl From<CreatedQueue> for CreateQueueResponse {
    fn from(queue: CreatedQueue) -> Self {
        Self {
            id: queue.id(),
            name: queue.name().to_owned(),
            visibility_timeout_seconds: queue.visibility_timeout_seconds(),
            max_delivery_attempts: queue.max_delivery_attempts(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct QueuePath {
    queue_name: String,
}

impl QueuePath {
    pub(super) fn into_queue_name(self) -> String {
        self.queue_name
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
    pub(super) fn into_command(self, queue_name: String) -> EnqueueMessageCommand {
        EnqueueMessageCommand::new(queue_name, self.payload, self.priority, self.ttl_seconds)
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
