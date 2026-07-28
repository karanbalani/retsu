use thiserror::Error;
use tracing::field;

use crate::modules::queue::{
    application::repository::{CreateQueueOutcome, QueueRepository},
    domain::{Queue, QueueDetails, QueueNameError, QueueSettingsError, QueueValidationError},
};

#[derive(Debug)]
pub(in crate::modules::queue) struct CreateQueueCommand {
    name: String,
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
    default_message_ttl_seconds: Option<u32>,
}

impl CreateQueueCommand {
    pub(in crate::modules::queue) fn new(
        name: String,
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
        default_message_ttl_seconds: Option<u32>,
    ) -> Self {
        Self {
            name,
            visibility_timeout_seconds,
            max_delivery_attempts,
            default_message_ttl_seconds,
        }
    }
}

#[tracing::instrument(name = "queue.create", skip_all, fields(queue.name = %command.name, queue.id = field::Empty), err)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: CreateQueueCommand,
) -> Result<QueueDetails, CreateQueueError>
where
    R: QueueRepository,
{
    let queue = Queue::new(
        command.name,
        command.visibility_timeout_seconds,
        command.max_delivery_attempts,
        command.default_message_ttl_seconds,
    )
    .map_err(CreateQueueError::from)?;

    match repository
        .create_queue(&queue)
        .await
        .map_err(CreateQueueError::Persistence)?
    {
        CreateQueueOutcome::Created => {}
        CreateQueueOutcome::AlreadyExists => return Err(CreateQueueError::AlreadyExists),
    }

    tracing::Span::current().record("queue.id", field::display(queue.id()));

    Ok(queue.details())
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum CreateQueueError {
    #[error(transparent)]
    InvalidName(QueueNameError),

    #[error(transparent)]
    InvalidSettings(QueueSettingsError),

    #[error("a queue with this name already exists")]
    AlreadyExists,

    #[error("failed to persist the queue")]
    Persistence(#[source] anyhow::Error),
}

impl From<QueueValidationError> for CreateQueueError {
    fn from(error: QueueValidationError) -> Self {
        match error {
            QueueValidationError::InvalidName(error) => Self::InvalidName(error),
            QueueValidationError::InvalidSettings(error) => Self::InvalidSettings(error),
        }
    }
}
