use std::path::Path;

use super::{PreparedEntrypoint, RuntimeProcess, prepare};
use crate::{
    cli::{Cli, Command, WorkerCommand},
    modules::WorkerResolutionError,
};

#[test]
fn lists_and_resolves_the_registered_workers() {
    let module_list = prepare(worker_cli(WorkerCommand::List { module: None }))
        .expect("worker modules should resolve");
    let queue_worker_list = prepare(worker_cli(WorkerCommand::List {
        module: Some("queue".to_owned()),
    }))
    .expect("queue workers should resolve");

    assert_eq!(prepared_output(module_list), "queue\n");
    assert_eq!(
        prepared_output(queue_worker_list),
        "dead-letter-message-cleaner\nexpired-message-cleaner\nstate-metrics-collector\n"
    );

    for worker_name in [
        "dead-letter-message-cleaner",
        "expired-message-cleaner",
        "state-metrics-collector",
    ] {
        let runtime = prepare(Cli {
            config: Some("custom.yaml".into()),
            command: Command::Worker {
                command: WorkerCommand::Run {
                    module: "queue".to_owned(),
                    name: worker_name.to_owned(),
                },
            },
        })
        .expect("known worker selection should resolve");

        let PreparedEntrypoint::Runtime(runtime) = runtime else {
            panic!("worker run should prepare a runtime entrypoint");
        };

        assert_eq!(runtime.config_path(), Some(Path::new("custom.yaml")));
        assert_eq!(runtime.process_mode(), "worker");
        assert_eq!(runtime.worker_selection(), Some(("queue", worker_name)));

        let RuntimeProcess::Worker(worker) = runtime.process else {
            panic!("worker run should prepare the selected worker");
        };
        assert_eq!(worker.into_registration().name, worker_name);
    }
}

#[test]
fn rejects_unknown_worker_selections_with_available_choices() {
    let unknown_module = resolution_error(WorkerCommand::Run {
        module: "billing".to_owned(),
        name: "collector".to_owned(),
    });

    assert!(matches!(
        unknown_module,
        WorkerResolutionError::UnknownModule { module, available }
            if module == "billing" && available == "queue"
    ));

    let unknown_worker = resolution_error(WorkerCommand::Run {
        module: "queue".to_owned(),
        name: "collector".to_owned(),
    });

    assert!(matches!(
        unknown_worker,
        WorkerResolutionError::UnknownWorker {
            module,
            name,
            available,
        } if module == "queue"
            && name == "collector"
            && available
                == "dead-letter-message-cleaner, expired-message-cleaner, state-metrics-collector"
    ));
}

fn worker_cli(command: WorkerCommand) -> Cli {
    Cli {
        config: None,
        command: Command::Worker { command },
    }
}

fn prepared_output(prepared: PreparedEntrypoint) -> String {
    let PreparedEntrypoint::Output(output) = prepared else {
        panic!("worker list should prepare output");
    };

    output
}

fn resolution_error(command: WorkerCommand) -> WorkerResolutionError {
    match prepare(worker_cli(command)) {
        Err(error) => error,
        Ok(_) => panic!("unknown worker selection should be rejected"),
    }
}
