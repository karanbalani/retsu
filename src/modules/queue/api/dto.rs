use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::application::{CreateQueueCommand, CreatedQueue};

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
