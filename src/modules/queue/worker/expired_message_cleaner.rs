use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::{app::ApplicationContext, worker::WorkerRegistration};

const PROCESSING_INTERVAL: Duration = Duration::from_secs(60);
const PROCESSING_BATCH_SIZE: u32 = 500;

pub(in crate::modules::queue) const NAME: &str = "expired-message-cleaner";

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

            result = context
                .queue_module()
                .process_expired_messages(PROCESSING_BATCH_SIZE) => result?
        };

        let processed = summary.processed();

        if processed > 0 {
            tracing::debug!(
                messages.processed = processed,
                messages.never_delivered = summary.never_delivered(),
                messages.previously_delivered = summary.previously_delivered(),
                "expired messages removed"
            );
        }

        if processed == u64::from(PROCESSING_BATCH_SIZE) {
            tokio::task::yield_now().await;
            continue;
        }

        tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            () = tokio::time::sleep(PROCESSING_INTERVAL) => {}
        }
    }
}
