use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::{app::ApplicationContext, worker::WorkerRegistration};

const REQUEUE_INTERVAL: Duration = Duration::from_secs(5);
const REQUEUE_BATCH_SIZE: u32 = 500;

pub(in crate::modules::queue) const NAME: &str = "visibility-timeout-processor";

pub(in crate::modules::queue) fn registration() -> WorkerRegistration {
    WorkerRegistration {
        name: NAME,
        run: Box::new(|context, cancellation| Box::pin(run(context, cancellation))),
    }
}

async fn run(context: ApplicationContext, cancellation: CancellationToken) -> anyhow::Result<()> {
    loop {
        let summary = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            result = context.queue_module().process_timed_out_messages(REQUEUE_BATCH_SIZE) => result?
        };

        let processed = summary.processed();

        if processed > 0 {
            tracing::debug!(
                messages.processed = processed,
                messages.requeued = summary.requeued(),
                messages.dead_lettered = summary.dead_lettered(),
                "timed-out messages processed"
            );
        }

        // if the batch was full, there may be more messages immediately available
        if processed == u64::from(REQUEUE_BATCH_SIZE) {
            tokio::task::yield_now().await;
            continue;
        }

        tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            () = tokio::time::sleep(REQUEUE_INTERVAL) => {}
        }
    }
}
