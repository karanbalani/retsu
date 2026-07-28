use thiserror::Error;
use uuid::Uuid;

use crate::modules::queue::{
    application::repository::QueueRepository,
    domain::{QueueConfigurationUpdate, QueueDetails, QueueSettingsError},
};

#[derive(Debug)]
pub(in crate::modules::queue) struct UpdateQueueCommand {
    queue_id: Uuid,
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
    default_message_ttl_seconds: Option<u32>,
}

impl UpdateQueueCommand {
    pub(in crate::modules::queue) fn new(
        queue_id: Uuid,
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
        default_message_ttl_seconds: Option<u32>,
    ) -> Self {
        Self {
            queue_id,
            visibility_timeout_seconds,
            max_delivery_attempts,
            default_message_ttl_seconds,
        }
    }
}

#[tracing::instrument(name = "queue.update", skip_all, fields(queue.id = %command.queue_id), err)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: UpdateQueueCommand,
) -> Result<QueueDetails, UpdateQueueError>
where
    R: QueueRepository,
{
    if command.visibility_timeout_seconds.is_none()
        && command.max_delivery_attempts.is_none()
        && command.default_message_ttl_seconds.is_none()
    {
        return Err(UpdateQueueError::NoConfigurationChanges);
    }

    let configuration = QueueConfigurationUpdate::new(
        command.visibility_timeout_seconds,
        command.max_delivery_attempts,
        command.default_message_ttl_seconds,
    )
    .map_err(UpdateQueueError::InvalidSettings)?;

    repository
        .update_queue(command.queue_id, &configuration)
        .await
        .map_err(UpdateQueueError::Persistence)?
        .ok_or(UpdateQueueError::QueueNotFound)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum UpdateQueueError {
    #[error("at least one queue configuration field must be provided")]
    NoConfigurationChanges,

    #[error(transparent)]
    InvalidSettings(QueueSettingsError),

    #[error("the requested queue does not exist")]
    QueueNotFound,

    #[error("failed to persist the queue configuration")]
    Persistence(#[source] anyhow::Error),
}
