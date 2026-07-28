use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use super::{
    CreateQueueCommand, CreateQueueError, CreateQueueOutcome, Queue, QueueRepository, execute,
};
use crate::modules::queue::domain::QueueDetails;

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

    async fn queue_details(
        &self,
        _queue_id: uuid::Uuid,
    ) -> Result<Option<QueueDetails>, anyhow::Error> {
        unreachable!("create queue tests should not read queue details")
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
