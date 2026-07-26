use thiserror::Error;
use tracing::field;
use uuid::Uuid;

use crate::modules::queue::{
    application::repository::{CreateQueueOutcome, QueueRepository},
    domain::{Queue, QueueNameError, QueueSettingsError, QueueValidationError},
};

#[derive(Debug)]
pub(in crate::modules::queue) struct CreateQueueCommand {
    name: String,
    visibility_timeout_seconds: Option<u32>,
    max_delivery_attempts: Option<u16>,
}

impl CreateQueueCommand {
    pub(in crate::modules::queue) fn new(
        name: String,
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
    ) -> Self {
        Self {
            name,
            visibility_timeout_seconds,
            max_delivery_attempts,
        }
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct CreatedQueue {
    id: Uuid,
    name: String,
    visibility_timeout_seconds: u32,
    max_delivery_attempts: u16,
}

impl CreatedQueue {
    pub(in crate::modules::queue) fn id(&self) -> Uuid {
        self.id
    }

    pub(in crate::modules::queue) fn name(&self) -> &str {
        &self.name
    }

    pub(in crate::modules::queue) fn visibility_timeout_seconds(&self) -> u32 {
        self.visibility_timeout_seconds
    }

    pub(in crate::modules::queue) fn max_delivery_attempts(&self) -> u16 {
        self.max_delivery_attempts
    }
}

#[tracing::instrument(name = "queue.create", skip_all, fields(queue.name = %command.name, queue.id = field::Empty), err)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: CreateQueueCommand,
) -> Result<CreatedQueue, CreateQueueError>
where
    R: QueueRepository,
{
    let queue = Queue::new(
        command.name,
        command.visibility_timeout_seconds,
        command.max_delivery_attempts,
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

    Ok(CreatedQueue {
        id: queue.id(),
        name: queue.name().to_owned(),
        visibility_timeout_seconds: queue.visibility_timeout_seconds(),
        max_delivery_attempts: queue.max_delivery_attempts(),
    })
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
