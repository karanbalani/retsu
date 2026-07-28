use thiserror::Error;
use tracing::field;
use uuid::Uuid;

use super::super::{
    application::repository::{DequeueMessageOutcome, QueueRepository},
    domain::MessagePriority,
};

#[derive(Debug)]
pub(in crate::modules::queue) struct DequeueMessageCommand {
    queue_id: Uuid,
}

impl DequeueMessageCommand {
    pub(in crate::modules::queue) fn new(queue_id: Uuid) -> Self {
        Self { queue_id }
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

pub(in crate::modules::queue) struct DequeueMessageResult {
    message: Option<DequeuedMessage>,
    queue_name: String,
    dead_lettered: u64,
}

impl DequeueMessageResult {
    pub(in crate::modules::queue) fn message(self) -> Option<DequeuedMessage> {
        self.message
    }

    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }

    pub(in crate::modules::queue) fn dead_lettered(&self) -> u64 {
        self.dead_lettered
    }
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
        queue.id = %command.queue_id,
        message.id = field::Empty,
        message.priority = field::Empty,
        message.delivery_attempts = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: DequeueMessageCommand,
) -> Result<DequeueMessageResult, DequeueMessageError>
where
    R: QueueRepository,
{
    let DequeueMessageCommand { queue_id } = command;
    let receipt_handle = Uuid::new_v4();

    match repository
        .dequeue_message(queue_id, receipt_handle)
        .await
        .map_err(DequeueMessageError::Persistence)?
    {
        DequeueMessageOutcome::Dequeued {
            id,
            payload,
            priority,
            receipt_handle,
            delivery_attempts,
            queue_name,
            dead_lettered,
        } => {
            tracing::Span::current().record("message.id", field::display(id));
            tracing::Span::current().record("message.priority", priority.as_str());
            tracing::Span::current().record("message.delivery_attempts", delivery_attempts);

            Ok(DequeueMessageResult {
                message: Some(DequeuedMessage {
                    id,
                    payload,
                    priority,
                    receipt_handle,
                    delivery_attempts,
                }),
                queue_name,
                dead_lettered,
            })
        }
        DequeueMessageOutcome::Empty {
            queue_name,
            dead_lettered,
        } => Ok(DequeueMessageResult {
            message: None,
            queue_name,
            dead_lettered,
        }),
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
