use thiserror::Error;
use uuid::Uuid;

use super::super::application::{QueueRepository, repository::AcknowledgeMessageOutcome};

#[derive(Debug)]
pub(in crate::modules::queue) struct AcknowledgeMessageCommand {
    queue_id: Uuid,
    message_id: Uuid,
    receipt_handle: Uuid,
}

impl AcknowledgeMessageCommand {
    pub(in crate::modules::queue) fn new(
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Self {
        Self {
            queue_id,
            message_id,
            receipt_handle,
        }
    }
}

pub(in crate::modules::queue) struct AcknowledgedMessage {
    queue_name: String,
}

impl AcknowledgedMessage {
    pub(in crate::modules::queue) fn queue_name(&self) -> &str {
        &self.queue_name
    }
}

#[tracing::instrument(
    name = "queue.acknowledge",
    skip_all,
    fields(
        queue.id = %command.queue_id,
        queue.name = tracing::field::Empty,
        message.id = %command.message_id,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    command: AcknowledgeMessageCommand,
) -> Result<Option<AcknowledgedMessage>, AcknowledgeMessageError>
where
    R: QueueRepository,
{
    let AcknowledgeMessageCommand {
        queue_id,
        message_id,
        receipt_handle,
    } = command;

    let queue_name = repository
        .queue_name(queue_id)
        .await
        .map_err(AcknowledgeMessageError::Persistence)?
        .ok_or(AcknowledgeMessageError::QueueNotFound)?;

    tracing::Span::current().record("queue.name", &queue_name);

    match repository
        .acknowledge_message(queue_id, message_id, receipt_handle)
        .await
        .map_err(AcknowledgeMessageError::Persistence)?
    {
        AcknowledgeMessageOutcome::Acknowledged => Ok(Some(AcknowledgedMessage { queue_name })),
        AcknowledgeMessageOutcome::Unchanged => Ok(None),
    }
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum AcknowledgeMessageError {
    #[error("the requested queue does not exist")]
    QueueNotFound,

    #[error("failed to acknowledge the message")]
    Persistence(#[source] anyhow::Error),
}
