use std::time::Duration;

use anyhow::Context as _;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::{
    app::ApplicationContext, modules::queue::QueueStateCollectorLease, worker::WorkerRegistration,
};

const COLLECTION_INTERVAL: Duration = Duration::from_secs(15);
const LEADERSHIP_RETRY_INTERVAL: Duration = Duration::from_secs(15);

pub(in crate::modules::queue) const NAME: &str = "state-metrics-collector";

pub(in crate::modules::queue) fn registration() -> WorkerRegistration {
    WorkerRegistration {
        name: NAME,
        run: Box::new(|context, cancellation| Box::pin(run(context, cancellation))),
    }
}

async fn run(context: ApplicationContext, cancellation: CancellationToken) -> anyhow::Result<()> {
    let Some(mut lease) = wait_for_leadership(&context, &cancellation).await? else {
        return Ok(());
    };

    tracing::info!("queue state metrics collector leadership acquired");

    collect(&context, &cancellation, &mut lease).await
}

async fn wait_for_leadership(
    context: &ApplicationContext,
    cancellation: &CancellationToken,
) -> anyhow::Result<Option<QueueStateCollectorLease>> {
    let mut interval = tokio::time::interval(LEADERSHIP_RETRY_INTERVAL);
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
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(COLLECTION_INTERVAL);
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
