use thiserror::Error;
use tracing::field;

use super::repository::{QueueRepository, TimeoutProcessingSummary};

#[tracing::instrument(
    name = "queue.visibility_timeout.process",
    parent = None,
    skip_all,
    fields(
        worker.operation = "queue_visibility_timeout_process",
        batch.size = batch_size,
        messages.processed = field::Empty,
        messages.requeued = field::Empty,
        messages.dead_lettered = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    batch_size: u32,
) -> Result<TimeoutProcessingSummary, ProcessTimedOutMessagesError>
where
    R: QueueRepository,
{
    let summary = repository
        .process_timed_out_messages(batch_size)
        .await
        .map_err(ProcessTimedOutMessagesError::Persistence)?;

    let span = tracing::Span::current();
    span.record("messages.processed", summary.processed());
    span.record("messages.requeued", summary.requeued());
    span.record("messages.dead_lettered", summary.dead_lettered());

    Ok(summary)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum ProcessTimedOutMessagesError {
    #[error("failed to requeue timed-out messages")]
    Persistence(#[source] anyhow::Error),
}
