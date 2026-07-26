use thiserror::Error;
use tracing::field;

use super::repository::MessageRepository;

#[tracing::instrument(
    name = "queue.visibility_timeout.requeue",
    parent = None,
    skip_all,
    fields(
        batch.size = batch_size,
        messages.requeued = field::Empty,
    ),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    batch_size: u32,
) -> Result<u64, RequeueTimedOutMessagesError>
where
    R: MessageRepository,
{
    let requeued = repository
        .requeue_timed_out_messages(batch_size)
        .await
        .map_err(RequeueTimedOutMessagesError::Persistence)?;

    tracing::Span::current().record("messages.requeued", requeued);

    Ok(requeued)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum RequeueTimedOutMessagesError {
    #[error("failed to requeue timed-out messages")]
    Persistence(#[source] anyhow::Error),
}
