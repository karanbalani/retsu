use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::{app::ApplicationContext, worker::WorkerRegistration};

const REQUEUE_INTERVAL: Duration = Duration::from_secs(1);
const REQUEUE_BATCH_SIZE: u32 = 500;

pub(super) fn registration() -> WorkerRegistration {
    WorkerRegistration {
        name: "queue_message_visibility_timeout_processor",
        run: Box::new(|context, cancellation| Box::pin(run(context, cancellation))),
    }
}

async fn run(context: ApplicationContext, cancellation: CancellationToken) -> anyhow::Result<()> {
    loop {
        let requeued = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            result = context.queue_module().requeue_timed_out_messages(REQUEUE_BATCH_SIZE) => result?
        };

        if requeued > 0 {
            tracing::debug!(messages.count = requeued, "timed-out messages requeued");
        }

        // if the batch was full, there may be more messages immediately available
        if requeued == u64::from(REQUEUE_BATCH_SIZE) {
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
