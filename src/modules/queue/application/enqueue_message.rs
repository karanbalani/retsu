use thiserror::Error;
use tracing::field;
use uuid::Uuid;

use super::super::{
    application::repository::{EnqueueMessageOutcome, QueueRepository},
    domain::{Message, MessagePriority, MessageValidationError},
};

#[derive(Debug)]
pub(in crate::modules::queue) struct EnqueueMessageCommand {
    queue_id: Uuid,
    payload: String,
    priority: String,
    ttl_seconds: Option<u32>,
}

impl EnqueueMessageCommand {
    pub(in crate::modules::queue) fn new(
        queue_id: Uuid,
        payload: String,
        priority: String,
        ttl_seconds: Option<u32>,
    ) -> Self {
        Self {
            queue_id,
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

#[tracing::instrument(name = "queue.enqueue", skip_all, fields(queue.id = %command.queue_id, queue.name = field::Empty, message.id = field::Empty, message.priority = field::Empty), err)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: EnqueueMessageCommand,
) -> Result<EnqueuedMessage, EnqueueMessageError>
where
    R: QueueRepository,
{
    let EnqueueMessageCommand {
        queue_id,
        payload,
        priority,
        ttl_seconds,
    } = command;

    let message =
        Message::new(payload, priority, ttl_seconds).map_err(EnqueueMessageError::from)?;

    let queue_name = repository
        .queue_name(queue_id)
        .await
        .map_err(EnqueueMessageError::Persistence)?
        .ok_or(EnqueueMessageError::QueueNotFound)?;

    tracing::Span::current().record("queue.name", &queue_name);
    tracing::Span::current().record("message.priority", message.priority().as_str());

    match repository
        .enqueue_message(queue_id, &message)
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
