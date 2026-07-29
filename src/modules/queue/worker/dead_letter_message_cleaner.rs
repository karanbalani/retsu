use tokio_util::sync::CancellationToken;

use crate::{
    app::ApplicationContext, configuration::DeadLetterMessageCleanerConfig,
    worker::WorkerRegistration,
};

pub(in crate::modules::queue) const NAME: &str = "dead-letter-message-cleaner";

pub(in crate::modules::queue) fn registration(
    configuration: &DeadLetterMessageCleanerConfig,
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
    configuration: DeadLetterMessageCleanerConfig,
) -> anyhow::Result<()> {
    loop {
        let summary = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            result = context.queue_module().purge_dead_letter_messages(
                configuration.retention(),
                configuration.batch_size,
            ) => result?
        };

        let purged = summary.purged();

        if purged > 0 {
            tracing::debug!(messages.purged = purged, "dead-letter messages purged");
        }

        if purged == u64::from(configuration.batch_size) {
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
