use anyhow::Context as _;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::{
    app::ApplicationContext, configuration::StateMetricsCollectorConfig,
    modules::queue::QueueStateCollectorLease, worker::WorkerRegistration,
};

pub(in crate::modules::queue) const NAME: &str = "state-metrics-collector";

pub(in crate::modules::queue) fn registration(
    configuration: &StateMetricsCollectorConfig,
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
    configuration: StateMetricsCollectorConfig,
) -> anyhow::Result<()> {
    let Some(mut lease) = wait_for_leadership(&context, &cancellation, &configuration).await?
    else {
        return Ok(());
    };

    tracing::info!("queue state metrics collector leadership acquired");

    collect(&context, &cancellation, &mut lease, &configuration).await
}

async fn wait_for_leadership(
    context: &ApplicationContext,
    cancellation: &CancellationToken,
    configuration: &StateMetricsCollectorConfig,
) -> anyhow::Result<Option<QueueStateCollectorLease>> {
    let mut interval = tokio::time::interval(configuration.leadership_retry_interval());
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(None),

            _ = interval.tick() => {}
        }

        let lease = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(None),

            result = context.queue_module().try_acquire_state_collector_lease() => {
                result.context("failed to acquire queue state metrics collector leadership")?
            }
        };

        if let Some(lease) = lease {
            return Ok(Some(lease));
        }

        tracing::info!("queue state metrics collector leadership held by another process");
    }
}

async fn collect(
    context: &ApplicationContext,
    cancellation: &CancellationToken,
    lease: &mut QueueStateCollectorLease,
    configuration: &StateMetricsCollectorConfig,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(configuration.collection_interval());
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            _ = interval.tick() => {}
        }

        let result = tokio::select! {
            biased;

            () = cancellation.cancelled() => return Ok(()),

            result = context.queue_module().refresh_state_metrics(lease) => result
        };

        result.context("failed to refresh queue state metrics while holding leadership")?;

        tracing::info!("queue state metrics refreshed");
    }
}
