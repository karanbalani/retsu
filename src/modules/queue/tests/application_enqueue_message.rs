use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use uuid::Uuid;

use super::{
    EnqueueMessageCommand, EnqueueMessageError, EnqueueMessageOutcome, Message, MessageRepository,
    MessageValidationError, QueueRepository, execute,
};
use crate::modules::queue::{
    application::repository::{
        AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome,
        TimeoutProcessingSummary,
    },
    domain::Queue,
};

struct FakeMessageRepository {
    enqueue_outcome: EnqueueMessageOutcome,
    enqueue_calls: AtomicUsize,
    enqueued_message: Mutex<Option<(Uuid, Message)>>,
}

struct FakeQueueRepository {
    name: Option<String>,
    name_calls: AtomicUsize,
}

impl FakeQueueRepository {
    fn existing(_queue_id: Uuid) -> Self {
        Self {
            name: Some("email-delivery".to_owned()),
            name_calls: AtomicUsize::new(0),
        }
    }

    fn missing() -> Self {
        Self {
            name: None,
            name_calls: AtomicUsize::new(0),
        }
    }
}

impl QueueRepository for FakeQueueRepository {
    async fn create_queue(&self, _queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        unreachable!("enqueue tests should not create queues")
    }

    async fn queue_name(&self, _queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        self.name_calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.name.clone())
    }
}

impl FakeMessageRepository {
    fn new(enqueue_outcome: EnqueueMessageOutcome) -> Self {
        Self {
            enqueue_outcome,
            enqueue_calls: AtomicUsize::new(0),
            enqueued_message: Mutex::new(None),
        }
    }

    fn enqueue_calls(&self) -> usize {
        self.enqueue_calls.load(Ordering::Relaxed)
    }
}

impl MessageRepository for FakeMessageRepository {
    async fn enqueue_message(
        &self,
        queue_id: Uuid,
        message: &Message,
    ) -> Result<EnqueueMessageOutcome, anyhow::Error> {
        self.enqueue_calls.fetch_add(1, Ordering::Relaxed);
        self.enqueued_message
            .lock()
            .expect("enqueued message lock should not be poisoned")
            .replace((queue_id, message.clone()));

        Ok(match self.enqueue_outcome {
            EnqueueMessageOutcome::Enqueued => EnqueueMessageOutcome::Enqueued,
            EnqueueMessageOutcome::QueueNotFound => EnqueueMessageOutcome::QueueNotFound,
        })
    }

    async fn dequeue_message(
        &self,
        _queue_id: Uuid,
        _receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error> {
        unreachable!("enqueue tests should not dequeue messages")
    }

    async fn acknowledge_message(
        &self,
        _queue_id: Uuid,
        _message_id: Uuid,
        _receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        unreachable!("enqueue tests should not acknowledge messages")
    }

    async fn process_timed_out_messages(
        &self,
        _batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, anyhow::Error> {
        unreachable!("this test should not process timed-out messages")
    }

    async fn process_expired_messages(
        &self,
        _batch_size: u32,
    ) -> Result<crate::modules::queue::ExpiredMessagesCleanupSummary, anyhow::Error> {
        unreachable!("this test should not process expired messages")
    }
}

#[tokio::test]
async fn persists_and_returns_the_effective_message() {
    let queue_id = Uuid::now_v7();
    let queues = FakeQueueRepository::existing(queue_id);
    let repository = FakeMessageRepository::new(EnqueueMessageOutcome::Enqueued);
    let command = EnqueueMessageCommand::new(
        queue_id,
        r#"{"job":42}"#.to_owned(),
        "HIGH".to_owned(),
        Some(60),
    );

    let enqueued = execute(&queues, &repository, command)
        .await
        .expect("valid message should be enqueued");

    assert_eq!(repository.enqueue_calls(), 1);
    assert_eq!(enqueued.queue_name(), "email-delivery");
    assert_eq!(enqueued.priority(), "HIGH");

    let persisted = repository
        .enqueued_message
        .lock()
        .expect("enqueued message lock should not be poisoned");
    let (persisted_queue_id, message) = persisted
        .as_ref()
        .expect("message should be sent to the repository");

    assert_eq!(*persisted_queue_id, queue_id);
    assert_eq!(message.id(), enqueued.id());
    assert_eq!(message.payload(), r#"{"job":42}"#);
    assert_eq!(message.priority().as_str(), enqueued.priority());
    assert_eq!(message.ttl_seconds(), Some(60));
}

#[tokio::test]
async fn rejects_invalid_messages_without_calling_the_repository() {
    let repository = FakeMessageRepository::new(EnqueueMessageOutcome::Enqueued);
    let queues = FakeQueueRepository::existing(Uuid::now_v7());
    let queue_id = Uuid::now_v7();

    let invalid_priority = execute(
        &queues,
        &repository,
        EnqueueMessageCommand::new(queue_id, "payload".to_owned(), "URGENT".to_owned(), None),
    )
    .await;
    assert!(matches!(
        invalid_priority,
        Err(EnqueueMessageError::InvalidMessage(
            MessageValidationError::InvalidPriority
        ))
    ));

    let invalid_ttl = execute(
        &queues,
        &repository,
        EnqueueMessageCommand::new(queue_id, "payload".to_owned(), "HIGH".to_owned(), Some(0)),
    )
    .await;
    assert!(matches!(
        invalid_ttl,
        Err(EnqueueMessageError::InvalidMessage(
            MessageValidationError::InvalidTtl
        ))
    ));

    assert_eq!(repository.enqueue_calls(), 0);
    assert_eq!(queues.name_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn reports_when_the_target_queue_does_not_exist() {
    let queues = FakeQueueRepository::missing();
    let repository = FakeMessageRepository::new(EnqueueMessageOutcome::Enqueued);
    let command = EnqueueMessageCommand::new(
        Uuid::now_v7(),
        "payload".to_owned(),
        "MEDIUM".to_owned(),
        None,
    );

    let result = execute(&queues, &repository, command).await;

    assert!(matches!(result, Err(EnqueueMessageError::QueueNotFound)));
    assert_eq!(repository.enqueue_calls(), 0);
    assert_eq!(queues.name_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn preserves_a_queue_not_found_outcome_from_the_message_write() {
    let queue_id = Uuid::now_v7();
    let queues = FakeQueueRepository::existing(queue_id);
    let repository = FakeMessageRepository::new(EnqueueMessageOutcome::QueueNotFound);
    let command =
        EnqueueMessageCommand::new(queue_id, "payload".to_owned(), "MEDIUM".to_owned(), None);

    let result = execute(&queues, &repository, command).await;

    assert!(matches!(result, Err(EnqueueMessageError::QueueNotFound)));
    assert_eq!(repository.enqueue_calls(), 1);
}
