use thiserror::Error;
use tracing::field;
use uuid::Uuid;

use super::super::{
    application::repository::{EnqueueMessageOutcome, MessageRepository},
    domain::{Message, MessagePriority, MessageValidationError},
};

#[derive(Debug)]
pub(in crate::modules::queue) struct EnqueueMessageCommand {
    queue_name: String,
    payload: String,
    priority: String,
    ttl_seconds: Option<u32>,
}

impl EnqueueMessageCommand {
    pub(in crate::modules::queue) fn new(
        queue_name: String,
        payload: String,
        priority: String,
        ttl_seconds: Option<u32>,
    ) -> Self {
        Self {
            queue_name,
            payload,
            priority,
            ttl_seconds,
        }
    }
}

#[derive(Debug)]
pub(in crate::modules::queue) struct EnqueuedMessage {
    id: Uuid,
    queue_name: String,
    priority: MessagePriority,
}

impl EnqueuedMessage {
    pub(in crate::modules::queue) fn id(&self) -> Uuid {
        self.id
    }

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn priority(&self) -> &'static str {
        self.priority.as_str()
    }
}

#[tracing::instrument(name = "queue.enqueue", skip_all, fields(queue.name = %command.queue_name, message.id = field::Empty, message.priority = field::Empty), err)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: EnqueueMessageCommand,
) -> Result<EnqueuedMessage, EnqueueMessageError>
where
    R: MessageRepository,
{
    let EnqueueMessageCommand {
        queue_name,
        payload,
        priority,
        ttl_seconds,
    } = command;

    let message =
        Message::new(payload, priority, ttl_seconds).map_err(EnqueueMessageError::from)?;

    tracing::Span::current().record("message.priority", message.priority().as_str());

    match repository
        .enqueue_message(&queue_name, &message)
        .await
        .map_err(EnqueueMessageError::Persistence)?
    {
        EnqueueMessageOutcome::Enqueued => {}
        EnqueueMessageOutcome::QueueNotFound => return Err(EnqueueMessageError::QueueNotFound),
    }

    tracing::Span::current().record("message.id", field::display(message.id()));

    Ok(EnqueuedMessage {
        id: message.id(),
        queue_name,
        priority: message.priority(),
    })
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum EnqueueMessageError {
    #[error(transparent)]
    InvalidMessage(#[from] MessageValidationError),

    #[error("the requested queue does not exist")]
    QueueNotFound,

    #[error("failed to persist the message")]
    Persistence(#[source] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use uuid::Uuid;

    use super::{
        EnqueueMessageCommand, EnqueueMessageError, EnqueueMessageOutcome, Message,
        MessageRepository, MessageValidationError, execute,
    };
    use crate::modules::queue::application::repository::{
        AcknowledgeMessageOutcome, DequeueMessageOutcome, TimeoutProcessingSummary,
    };

    struct FakeMessageRepository {
        enqueue_outcome: EnqueueMessageOutcome,
        enqueue_calls: AtomicUsize,
        enqueued_message: Mutex<Option<(String, Message)>>,
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
            queue_name: &str,
            message: &Message,
        ) -> Result<EnqueueMessageOutcome, anyhow::Error> {
            self.enqueue_calls.fetch_add(1, Ordering::Relaxed);
            self.enqueued_message
                .lock()
                .expect("enqueued message lock should not be poisoned")
                .replace((queue_name.to_owned(), message.clone()));

            Ok(match self.enqueue_outcome {
                EnqueueMessageOutcome::Enqueued => EnqueueMessageOutcome::Enqueued,
                EnqueueMessageOutcome::QueueNotFound => EnqueueMessageOutcome::QueueNotFound,
            })
        }

        async fn dequeue_message(
            &self,
            _queue_name: &str,
            _receipt_handle: Uuid,
        ) -> Result<DequeueMessageOutcome, anyhow::Error> {
            unreachable!("enqueue tests should not dequeue messages")
        }

        async fn acknowledge_message(
            &self,
            _queue_name: &str,
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
    }

    #[tokio::test]
    async fn persists_and_returns_the_effective_message() {
        let repository = FakeMessageRepository::new(EnqueueMessageOutcome::Enqueued);
        let command = EnqueueMessageCommand::new(
            "email-delivery".to_owned(),
            r#"{"job":42}"#.to_owned(),
            "HIGH".to_owned(),
            Some(60),
        );

        let enqueued = execute(&repository, command)
            .await
            .expect("valid message should be enqueued");

        assert_eq!(repository.enqueue_calls(), 1);
        assert_eq!(enqueued.queue_name(), "email-delivery");
        assert_eq!(enqueued.priority(), "HIGH");

        let persisted = repository
            .enqueued_message
            .lock()
            .expect("enqueued message lock should not be poisoned");
        let (queue_name, message) = persisted
            .as_ref()
            .expect("message should be sent to the repository");

        assert_eq!(queue_name, enqueued.queue_name());
        assert_eq!(message.id(), enqueued.id());
        assert_eq!(message.payload(), r#"{"job":42}"#);
        assert_eq!(message.priority().as_str(), enqueued.priority());
        assert_eq!(message.ttl_seconds(), Some(60));
    }

    #[tokio::test]
    async fn rejects_invalid_messages_without_calling_the_repository() {
        let repository = FakeMessageRepository::new(EnqueueMessageOutcome::Enqueued);

        let invalid_priority = execute(
            &repository,
            EnqueueMessageCommand::new(
                "email-delivery".to_owned(),
                "payload".to_owned(),
                "URGENT".to_owned(),
                None,
            ),
        )
        .await;
        assert!(matches!(
            invalid_priority,
            Err(EnqueueMessageError::InvalidMessage(
                MessageValidationError::InvalidPriority
            ))
        ));

        let invalid_ttl = execute(
            &repository,
            EnqueueMessageCommand::new(
                "email-delivery".to_owned(),
                "payload".to_owned(),
                "HIGH".to_owned(),
                Some(0),
            ),
        )
        .await;
        assert!(matches!(
            invalid_ttl,
            Err(EnqueueMessageError::InvalidMessage(
                MessageValidationError::InvalidTtl
            ))
        ));

        assert_eq!(repository.enqueue_calls(), 0);
    }

    #[tokio::test]
    async fn reports_when_the_target_queue_does_not_exist() {
        let repository = FakeMessageRepository::new(EnqueueMessageOutcome::QueueNotFound);
        let command = EnqueueMessageCommand::new(
            "missing-queue".to_owned(),
            "payload".to_owned(),
            "MEDIUM".to_owned(),
            None,
        );

        let result = execute(&repository, command).await;

        assert!(matches!(result, Err(EnqueueMessageError::QueueNotFound)));
        assert_eq!(repository.enqueue_calls(), 1);
    }
}
