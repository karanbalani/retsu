use thiserror::Error;
use uuid::Uuid;

use super::super::application::{MessageRepository, repository::AcknowledgeMessageOutcome};

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

    pub(in crate::modules::queue) fn queue_id(&self) -> Uuid {
        self.queue_id
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
) -> Result<(), AcknowledgeMessageError>
where
    R: MessageRepository,
{
    let AcknowledgeMessageCommand {
        queue_id,
        message_id,
        receipt_handle,
    } = command;

    match repository
        .acknowledge_message(queue_id, message_id, receipt_handle)
        .await
        .map_err(AcknowledgeMessageError::Persistence)?
    {
        AcknowledgeMessageOutcome::Acknowledged => Ok(()),

        AcknowledgeMessageOutcome::QueueNotFound => Err(AcknowledgeMessageError::QueueNotFound),

        AcknowledgeMessageOutcome::MessageNotFound => Err(AcknowledgeMessageError::MessageNotFound),

        AcknowledgeMessageOutcome::ReceiptHandleInvalid => {
            Err(AcknowledgeMessageError::ReceiptHandleInvalid)
        }
    }
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum AcknowledgeMessageError {
    #[error("the requested queue does not exist")]
    QueueNotFound,

    #[error("the requested message does not exist in this queue")]
    MessageNotFound,

    #[error("the receipt handle is not valid for the message's current unexpired delivery attempt")]
    ReceiptHandleInvalid,

    #[error("failed to acknowledge the message")]
    Persistence(#[source] anyhow::Error),
}
