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

#[derive(Debug)]
pub(in crate::modules::queue) struct CreatedQueue {
    id: Uuid,
    name: String,
    visibility_timeout_seconds: u32,
    max_delivery_attempts: u16,
    default_message_ttl_seconds: u32,
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

    pub(in crate::modules::queue) fn default_message_ttl_seconds(&self) -> u32 {
        self.default_message_ttl_seconds
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

    Ok(CreatedQueue {
        id: queue.id(),
        name: queue.name().to_owned(),
        visibility_timeout_seconds: queue.visibility_timeout_seconds(),
        max_delivery_attempts: queue.max_delivery_attempts(),
        default_message_ttl_seconds: queue.default_message_ttl_seconds(),
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{
        CreateQueueCommand, CreateQueueError, CreateQueueOutcome, Queue, QueueRepository, execute,
    };

    struct FakeQueueRepository {
        outcome: CreateQueueOutcome,
        calls: AtomicUsize,
        persisted_queue: Mutex<Option<Queue>>,
    }

    impl FakeQueueRepository {
        fn new(outcome: CreateQueueOutcome) -> Self {
            Self {
                outcome,
                calls: AtomicUsize::new(0),
                persisted_queue: Mutex::new(None),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl QueueRepository for FakeQueueRepository {
        async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.persisted_queue
                .lock()
                .expect("persisted queue lock should not be poisoned")
                .replace(queue.clone());

            Ok(match self.outcome {
                CreateQueueOutcome::Created => CreateQueueOutcome::Created,
                CreateQueueOutcome::AlreadyExists => CreateQueueOutcome::AlreadyExists,
            })
        }
    }

    #[tokio::test]
    async fn persists_and_returns_the_effective_queue() {
        let repository = FakeQueueRepository::new(CreateQueueOutcome::Created);
        let command =
            CreateQueueCommand::new("email-delivery".to_owned(), Some(45), Some(7), Some(300));

        let created = execute(&repository, command)
            .await
            .expect("valid queue should be created");

        assert_eq!(repository.calls(), 1);
        assert_eq!(created.name(), "email-delivery");
        assert_eq!(created.visibility_timeout_seconds(), 45);
        assert_eq!(created.max_delivery_attempts(), 7);

        let persisted_queue = repository
            .persisted_queue
            .lock()
            .expect("persisted queue lock should not be poisoned");
        let persisted_queue = persisted_queue
            .as_ref()
            .expect("queue should be sent to the repository");

        assert_eq!(persisted_queue.id(), created.id());
        assert_eq!(persisted_queue.name(), created.name());
    }

    #[tokio::test]
    async fn rejects_invalid_commands_without_calling_the_repository() {
        let repository = FakeQueueRepository::new(CreateQueueOutcome::Created);
        let command = CreateQueueCommand::new("Invalid Name".to_owned(), None, None, None);

        let result = execute(&repository, command).await;

        assert!(matches!(result, Err(CreateQueueError::InvalidName(_))));
        assert_eq!(repository.calls(), 0);
    }

    #[tokio::test]
    async fn maps_repository_conflicts_to_queue_already_exists() {
        let repository = FakeQueueRepository::new(CreateQueueOutcome::AlreadyExists);
        let command = CreateQueueCommand::new("email-delivery".to_owned(), None, None, None);

        let result = execute(&repository, command).await;

        assert!(matches!(result, Err(CreateQueueError::AlreadyExists)));
        assert_eq!(repository.calls(), 1);
    }
}
