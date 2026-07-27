use tokio::task::JoinSet;

use super::{WorkerExit, drain, unexpected_worker_exit};

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

    let error =
        unexpected_worker_exit(Some(Err(join_error))).expect_err("worker panic should propagate");

    assert_eq!(error.to_string(), "background worker task panicked");
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
