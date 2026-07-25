mod management;
mod registration;

pub(crate) use management::registration as management_registration;
pub(crate) use registration::WorkerRegistration;

use std::time::Duration;

use anyhow::{Context, anyhow};
use tokio::task::{JoinError, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::app::ApplicationContext;

struct WorkerExit {
    name: &'static str,
    result: anyhow::Result<()>,
}

#[tracing::instrument(name = "worker.serve", skip_all)]
pub(crate) async fn serve(
    context: &ApplicationContext,
    registrations: Vec<WorkerRegistration>,
    shutdown_timeout: Duration,
) -> anyhow::Result<()> {
    if registrations.is_empty() {
        tracing::warn!("no background workers registered");
        crate::shutdown::signal().await?;
        return Ok(());
    }

    let cancellation = CancellationToken::new();
    let mut tasks = JoinSet::new();

    for registration in registrations {
        let name = registration.name;
        let run = registration.run;
        let context = context.clone();
        let cancellation = cancellation.child_token();

        let span = tracing::info_span!("background.worker", worker.name = name);

        tasks.spawn(
            async move {
                tracing::info!("background worker started");
                let result = (run)(context, cancellation).await;
                WorkerExit { name, result }
            }
            .instrument(span),
        );
    }

    let result = tokio::select! {
        signal_result = crate::shutdown::signal() => {
            signal_result.map_err(anyhow::Error::from)
        }
        worker_result = tasks.join_next() => {
            unexpected_worker_exit(worker_result)
        }
    };

    cancellation.cancel();

    let drain_result = match tokio::time::timeout(shutdown_timeout, drain(&mut tasks)).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(
                timeout_seconds = shutdown_timeout.as_secs(),
                "worker shutdown timed out; aborting remaining workers"
            );
            tasks.shutdown().await;
            Ok(())
        }
    };

    result?;

    drain_result
}

fn unexpected_worker_exit(
    worker_result: Option<Result<WorkerExit, JoinError>>,
) -> anyhow::Result<()> {
    let worker_exit = worker_result
        .context("worker task set became empty unexpectedly")?
        .context("background worker task panicked")?;

    worker_exit
        .result
        .with_context(|| format!("background worker `{}` failed", worker_exit.name))?;

    Err(anyhow!(
        "background worker `{}` exited unexpectedly",
        worker_exit.name
    ))
}

async fn drain(tasks: &mut JoinSet<WorkerExit>) -> anyhow::Result<()> {
    let mut first_error = None;

    while let Some(worker_result) = tasks.join_next().await {
        match worker_result {
            Ok(WorkerExit {
                name,
                result: Ok(()),
            }) => {
                tracing::info!(worker.name = name, "background worker stopped")
            }

            Ok(WorkerExit {
                name,
                result: Err(error),
            }) => {
                let error =
                    error.context(format!("background worker `{name}` failed during shutdown"));

                tracing::error!(worker.name = name, error = %error, "background worker failed");

                first_error.get_or_insert(error);
            }

            Err(error) => {
                let error = anyhow::Error::new(error)
                    .context("background worker task panicked during shutdown");

                tracing::error!(error = %error, "background worker task failed");

                first_error.get_or_insert(error);
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinSet;

    use super::{WorkerExit, drain, unexpected_worker_exit};

    #[test]
    fn reports_an_empty_worker_task_set() {
        let error = unexpected_worker_exit(None).expect_err("empty task set should fail");

        assert_eq!(
            error.to_string(),
            "worker task set became empty unexpectedly"
        );
    }

    #[test]
    fn reports_a_worker_that_exits_successfully_as_unexpected() {
        let worker = WorkerExit {
            name: "scheduler",
            result: Ok(()),
        };

        let error = unexpected_worker_exit(Some(Ok(worker))).expect_err("worker exit should fail");

        assert_eq!(
            error.to_string(),
            "background worker `scheduler` exited unexpectedly"
        );
    }

    #[test]
    fn preserves_a_worker_failure_in_the_error_chain() {
        let worker = WorkerExit {
            name: "scheduler",
            result: Err(anyhow::anyhow!("queue lease failed")),
        };

        let error =
            unexpected_worker_exit(Some(Ok(worker))).expect_err("worker failure should propagate");

        assert_eq!(
            format!("{error:#}"),
            "background worker `scheduler` failed: queue lease failed"
        );
    }

    #[tokio::test]
    async fn maps_worker_panics_to_a_stable_context() {
        let handle = tokio::spawn(async {
            panic!("worker panic");
        });
        let join_error = handle.await.expect_err("task should panic");

        let error = unexpected_worker_exit(Some(Err(join_error)))
            .expect_err("worker panic should propagate");

        assert_eq!(error.to_string(), "background worker task panicked");
    }

    #[tokio::test]
    async fn drains_all_successful_workers() {
        let mut tasks = JoinSet::new();
        tasks.spawn(async {
            WorkerExit {
                name: "first",
                result: Ok(()),
            }
        });
        tasks.spawn(async {
            WorkerExit {
                name: "second",
                result: Ok(()),
            }
        });

        drain(&mut tasks)
            .await
            .expect("successful workers should drain");

        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn returns_a_named_worker_failure_after_drain() {
        let mut tasks = JoinSet::new();
        tasks.spawn(async {
            WorkerExit {
                name: "scheduler",
                result: Err(anyhow::anyhow!("shutdown failed")),
            }
        });
        tasks.spawn(async {
            WorkerExit {
                name: "cleanup",
                result: Ok(()),
            }
        });

        let error = drain(&mut tasks)
            .await
            .expect_err("worker failure should be returned");

        assert_eq!(
            format!("{error:#}"),
            "background worker `scheduler` failed during shutdown: shutdown failed"
        );
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn returns_a_stable_error_when_a_worker_panics_during_drain() {
        let mut tasks: JoinSet<WorkerExit> = JoinSet::new();
        tasks.spawn(async {
            panic!("worker panic");
        });

        let error = drain(&mut tasks)
            .await
            .expect_err("worker panic should be returned");

        assert_eq!(
            error.to_string(),
            "background worker task panicked during shutdown"
        );
        assert!(tasks.is_empty());
    }
}
