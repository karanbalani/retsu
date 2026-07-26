use thiserror::Error;

use super::repository::MessageRepository;

#[tracing::instrument(
    name = "queue.visibility_timeout.requeue",
    skip_all,
    fields(batch.size = batch_size),
    err
)]
pub(in crate::modules::queue) async fn execute<R>(
    repository: &R,
    batch_size: u32,
) -> Result<u64, RequeueTimedOutMessagesError>
where
    R: MessageRepository,
{
    repository
        .requeue_timed_out_messages(batch_size)
        .await
        .map_err(RequeueTimedOutMessagesError::Persistence)
}

#[derive(Debug, Error)]
pub(in crate::modules::queue) enum RequeueTimedOutMessagesError {
    #[error("failed to requeue timed-out messages")]
    Persistence(#[source] anyhow::Error),
}
