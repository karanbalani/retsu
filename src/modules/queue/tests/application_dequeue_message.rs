use std::sync::Mutex;

use uuid::{Uuid, Version};

use super::{
    DequeueMessageCommand, DequeueMessageError, DequeueMessageOutcome, MessagePriority,
    MessageRepository, execute,
};
use crate::modules::queue::{
    application::repository::{
        AcknowledgeMessageOutcome, EnqueueMessageOutcome, TimeoutProcessingSummary,
    },
    domain::Message,
};

enum FakeDequeueOutcome {
    Dequeued {
        id: Uuid,
        payload: String,
        priority: MessagePriority,
        delivery_attempts: u16,
    },
    Empty,
    QueueNotFound,
}

struct FakeMessageRepository {
    dequeue_outcome: FakeDequeueOutcome,
    dequeue_call: Mutex<Option<(String, Uuid)>>,
}

impl FakeMessageRepository {
    fn new(dequeue_outcome: FakeDequeueOutcome) -> Self {
        Self {
            dequeue_outcome,
            dequeue_call: Mutex::new(None),
        }
    }
}

impl MessageRepository for FakeMessageRepository {
    async fn enqueue_message(
        &self,
        _queue_name: &str,
        _message: &Message,
    ) -> Result<EnqueueMessageOutcome, anyhow::Error> {
        unreachable!("dequeue tests should not enqueue messages")
    }

    async fn dequeue_message(
        &self,
        queue_name: &str,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error> {
        self.dequeue_call
            .lock()
            .expect("dequeue call lock should not be poisoned")
            .replace((queue_name.to_owned(), receipt_handle));

        Ok(match &self.dequeue_outcome {
            FakeDequeueOutcome::Dequeued {
                id,
                payload,
                priority,
                delivery_attempts,
            } => DequeueMessageOutcome::Dequeued {
                id: *id,
                payload: payload.clone(),
                priority: *priority,
                receipt_handle,
                delivery_attempts: *delivery_attempts,
            },
            FakeDequeueOutcome::Empty => DequeueMessageOutcome::Empty,
            FakeDequeueOutcome::QueueNotFound => DequeueMessageOutcome::QueueNotFound,
        })
    }

    async fn acknowledge_message(
        &self,
        _queue_name: &str,
        _message_id: Uuid,
        _receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        unreachable!("dequeue tests should not acknowledge messages")
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
async fn returns_the_repository_lease_with_its_generated_receipt_handle() {
    let message_id = Uuid::now_v7();
    let repository = FakeMessageRepository::new(FakeDequeueOutcome::Dequeued {
        id: message_id,
        payload: r#"{"job":42}"#.to_owned(),
        priority: MessagePriority::High,
        delivery_attempts: 2,
    });

    let dequeued = execute(
        &repository,
        DequeueMessageCommand::new("email-delivery".to_owned()),
    )
    .await
    .expect("dequeue should succeed")
    .expect("repository returned a message");

    let call = repository
        .dequeue_call
        .lock()
        .expect("dequeue call lock should not be poisoned");
    let (queue_name, receipt_handle) = call.as_ref().expect("repository should be called once");

    assert_eq!(queue_name, "email-delivery");
    assert_eq!(receipt_handle.get_version(), Some(Version::Random));
    assert_eq!(dequeued.id(), message_id);
    assert_eq!(dequeued.payload(), r#"{"job":42}"#);
    assert_eq!(dequeued.priority(), "HIGH");
    assert_eq!(dequeued.receipt_handle(), *receipt_handle);
    assert_eq!(dequeued.delivery_attempts(), 2);
}

#[tokio::test]
async fn distinguishes_an_empty_queue_from_a_missing_queue() {
    let empty_repository = FakeMessageRepository::new(FakeDequeueOutcome::Empty);
    let empty = execute(
        &empty_repository,
        DequeueMessageCommand::new("email-delivery".to_owned()),
    )
    .await;
    assert!(matches!(empty, Ok(None)));

    let missing_repository = FakeMessageRepository::new(FakeDequeueOutcome::QueueNotFound);
    let missing = execute(
        &missing_repository,
        DequeueMessageCommand::new("missing-queue".to_owned()),
    )
    .await;
    assert!(matches!(missing, Err(DequeueMessageError::QueueNotFound)));
}
