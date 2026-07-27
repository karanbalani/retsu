use std::time::Duration;

use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

use crate::{app::ApplicationContext, worker::WorkerRegistration};

const COLLECTION_INTERVAL: Duration = Duration::from_secs(15);

pub(in crate::modules::queue) const NAME: &str = "state-metrics-collector";

pub(in crate::modules::queue) fn registration() -> WorkerRegistration {
    WorkerRegistration {
        name: NAME,
        run: Box::new(|context, cancellation| Box::pin(run(context, cancellation))),
    }
}

async fn run(context: ApplicationContext, cancellation: CancellationToken) -> anyhow::Result<()> {
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

            result = context.queue_module().refresh_state_metrics() => result
        };

        match result {
            Ok(()) => {
                tracing::debug!("queue state metrics refreshed");
            }

            Err(error) => {
                tracing::error!(
                    error = %error,
                    "queue state metrics refresh failed"
                );
            }
        }
    }
}
