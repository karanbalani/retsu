use tokio_util::sync::CancellationToken;

use crate::{
    app::ApplicationContext, configuration::ExpiredMessageCleanerConfig, worker::WorkerRegistration,
};

pub(in crate::modules::queue) const NAME: &str = "expired-message-cleaner";

pub(in crate::modules::queue) fn registration(
    configuration: &ExpiredMessageCleanerConfig,
) -> WorkerRegistration {
    let configuration = *configuration;

    WorkerRegistration {
        name: NAME,
        run: Box::new(move |context, cancellation| {
            Box::pin(run(context, cancellation, configuration))
        }),
    }
}

async fn run(
    context: ApplicationContext,
    cancellation: CancellationToken,
    configuration: ExpiredMessageCleanerConfig,
) -> anyhow::Result<()> {
    loop {
        let summary = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            result = context
                .queue_module()
                .process_expired_messages(configuration.batch_size) => result?
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

        if processed == u64::from(configuration.batch_size) {
            tokio::select! {
                biased;

                () = cancellation.cancelled() => return Ok(()),

                () = tokio::time::sleep(configuration.saturated_batch_delay()) => {}
            }

            continue;
        }

        tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            () = tokio::time::sleep(configuration.processing_interval()) => {}
        }
    }
}
