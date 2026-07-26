use thiserror::Error;
use tracing::field;
use uuid::Uuid;

use super::super::{
    application::repository::{DequeueMessageOutcome, MessageRepository},
    domain::MessagePriority,
};

#[derive(Debug)]
pub(in crate::modules::queue) struct DequeueMessageCommand {
    queue_name: String,
}

impl DequeueMessageCommand {
    pub(in crate::modules::queue) fn new(queue_name: String) -> Self {
        Self { queue_name }
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct DequeuedMessage {
    id: Uuid,
    payload: String,
    priority: MessagePriority,
    receipt_handle: Uuid,
    delivery_attempts: u16,
}

impl DequeuedMessage {
    pub(in crate::modules::queue) fn id(&self) -> Uuid {
        self.id
    }

    pub(in crate::modules::queue) fn payload(&self) -> &str {
        &self.payload
    }

    pub(in crate::modules::queue) fn priority(&self) -> &'static str {
        self.priority.as_str()
    }

    pub(in crate::modules::queue) fn receipt_handle(&self) -> Uuid {
        self.receipt_handle
    }

    pub(in crate::modules::queue) fn delivery_attempts(&self) -> u16 {
        self.delivery_attempts
    }
}

#[tracing::instrument(
    name = "queue.dequeue",
    skip_all,
    fields(
        queue.name = %command.queue_name,
        message.id = field::Empty,
        message.priority = field::Empty,
        message.delivery_attempts = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: DequeueMessageCommand,
) -> Result<Option<DequeuedMessage>, DequeueMessageError>
where
    R: MessageRepository,
{
    let DequeueMessageCommand { queue_name } = command;
    let receipt_handle = Uuid::new_v4();

    match repository
        .dequeue_message(&queue_name, receipt_handle)
        .await
        .map_err(DequeueMessageError::Persistence)?
    {
        DequeueMessageOutcome::Dequeued {
            id,
            payload,
            priority,
            receipt_handle,
            delivery_attempts,
        } => {
            tracing::Span::current().record("message.id", field::display(id));
            tracing::Span::current().record("message.priority", priority.as_str());
            tracing::Span::current().record("message.delivery_attempts", delivery_attempts);

            Ok(Some(DequeuedMessage {
                id,
                payload,
                priority,
                receipt_handle,
                delivery_attempts,
            }))
        }
        DequeueMessageOutcome::Empty => Ok(None),
        DequeueMessageOutcome::QueueNotFound => Err(DequeueMessageError::QueueNotFound),
    }
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum DequeueMessageError {
    #[error("the requested queue does not exist")]
    QueueNotFound,

    #[error("failed to dequeue a message")]
    Persistence(#[source] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use uuid::{Uuid, Version};

    use super::{
        DequeueMessageCommand, DequeueMessageError, DequeueMessageOutcome, MessagePriority,
        MessageRepository, execute,
    };
    use crate::modules::queue::{application::repository::EnqueueMessageOutcome, domain::Message};

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
}
