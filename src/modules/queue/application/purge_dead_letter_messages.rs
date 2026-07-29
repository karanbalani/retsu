use std::{num::TryFromIntError, time::Duration};

use thiserror::Error;
use tracing::field;

use super::repository::{DeadLetterMessagesPurgeSummary, QueueRepository};

#[tracing::instrument(
    name = "queue.dead_letter.purge",
    parent = None,
    skip_all,
    fields(
        worker.operation = "queue_dead_letter_purge",
        batch.size = batch_size,
        retention.seconds = retention.as_secs(),
        messages.purged = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    retention: Duration,
    batch_size: u32,
) -> Result<DeadLetterMessagesPurgeSummary, PurgeDeadLetterMessagesError>
where
    R: QueueRepository,
{
    let retention_seconds = i64::try_from(retention.as_secs())
        .map_err(PurgeDeadLetterMessagesError::RetentionOverflow)?;

    let summary = repository
        .purge_dead_letter_messages(retention_seconds, batch_size)
        .await
        .map_err(PurgeDeadLetterMessagesError::Persistence)?;

    tracing::Span::current().record("messages.purged", summary.purged());

    Ok(summary)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum PurgeDeadLetterMessagesError {
    #[error("dead-letter message retention exceeds the supported range")]
    RetentionOverflow(#[source] TryFromIntError),

    #[error("failed to purge retained dead-letter messages")]
    Persistence(#[source] anyhow::Error),
}
