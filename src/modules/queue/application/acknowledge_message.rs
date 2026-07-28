use thiserror::Error;
use uuid::Uuid;

use super::super::application::{
    MessageRepository, QueueRepository, repository::AcknowledgeMessageOutcome,
};

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
pub(in crate::modules::queue) async fn execute<Q, M>(
    queue_repository: &Q,
    message_repository: &M,
    command: AcknowledgeMessageCommand,
) -> Result<AcknowledgedMessage, AcknowledgeMessageError>
where
    Q: QueueRepository,
    M: MessageRepository,
{
    let AcknowledgeMessageCommand {
        queue_id,
        message_id,
        receipt_handle,
    } = command;

    let queue = queue_repository
        .queue_details(queue_id)
        .await
        .map_err(AcknowledgeMessageError::Persistence)?
        .ok_or(AcknowledgeMessageError::QueueNotFound)?;

    tracing::Span::current().record("queue.name", queue.name());

    match message_repository
        .acknowledge_message(queue_id, message_id, receipt_handle)
        .await
        .map_err(AcknowledgeMessageError::Persistence)?
    {
        AcknowledgeMessageOutcome::Acknowledged => Ok(AcknowledgedMessage {
            queue_name: queue.name().to_owned(),
        }),

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
