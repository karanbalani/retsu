use thiserror::Error;
use tracing::field;

use super::repository::{ExpiredMessagesCleanupSummary, MessageRepository};

#[tracing::instrument(
    name = "queue.expiration.process",
    parent = None,
    skip_all,
    fields(
        worker.operation = "queue_expiration_process",
        batch.size = batch_size,
        messages.processed = field::Empty,
        messages.never_delivered = field::Empty,
        messages.previously_delivered = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    batch_size: u32,
) -> Result<ExpiredMessagesCleanupSummary, ProcessExpiredMessagesError>
where
    R: MessageRepository,
{
    let summary = repository
        .process_expired_messages(batch_size)
        .await
        .map_err(ProcessExpiredMessagesError::Persistence)?;

    let span = tracing::Span::current();
    span.record("messages.processed", summary.processed());
    span.record("messages.never_delivered", summary.never_delivered());
    span.record(
        "messages.previously_delivered",
        summary.previously_delivered(),
    );

    Ok(summary)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum ProcessExpiredMessagesError {
    #[error("failed to remove expired messages")]
    Persistence(#[source] anyhow::Error),
}
