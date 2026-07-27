use std::path::{Path, PathBuf};

use crate::{
    cli::{Cli, Command, WorkerCommand},
    configuration::AppConfiguration,
    modules::{self, ResolvedWorker, WorkerResolutionError},
    observability::Metrics,
};

pub(crate) enum PreparedEntrypoint {
    Output(String),
    Runtime(RuntimeEntrypoint),
}

pub(crate) struct RuntimeEntrypoint {
    config_path: Option<PathBuf>,
    process: RuntimeProcess,
}

enum RuntimeProcess {
    Api,
    Worker(ResolvedWorker),
    Migrate,
}

impl RuntimeEntrypoint {
    pub(crate) fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    pub(crate) const fn process_mode(&self) -> &'static str {
        match self.process {
            RuntimeProcess::Api => "api",
            RuntimeProcess::Worker(_) => "worker",
            RuntimeProcess::Migrate => "migrate",
        }
    }

    pub(crate) fn worker_selection(&self) -> Option<(&'static str, &'static str)> {
        match &self.process {
            RuntimeProcess::Worker(worker) => Some((worker.module_name(), worker.worker_name())),

            RuntimeProcess::Api | RuntimeProcess::Migrate => None,
        }
    }

    pub(crate) async fn run(
        self,
        configuration: AppConfiguration,
        metrics: Metrics,
    ) -> anyhow::Result<()> {
        match self.process {
            RuntimeProcess::Api => super::api::run(configuration, metrics).await,

            RuntimeProcess::Worker(worker) => {
                super::worker::run(configuration, metrics, worker.into_registration()).await
            }

            RuntimeProcess::Migrate => super::migrate::run(configuration).await,
        }
    }
}

pub(crate) fn prepare(cli: Cli) -> Result<PreparedEntrypoint, WorkerResolutionError> {
    let Cli { config, command } = cli;

    match command {
        Command::Api => Ok(PreparedEntrypoint::Runtime(RuntimeEntrypoint {
            config_path: config,
            process: RuntimeProcess::Api,
        })),

        Command::Migrate => Ok(PreparedEntrypoint::Runtime(RuntimeEntrypoint {
            config_path: config,
            process: RuntimeProcess::Migrate,
        })),

        Command::Worker {
            command: WorkerCommand::List { module },
        } => {
            let output = worker_list_output(module.as_deref())?;

            Ok(PreparedEntrypoint::Output(output))
        }

        Command::Worker {
            command: WorkerCommand::Run { module, name },
        } => {
            let worker = modules::resolve_worker(&module, &name)?;

            Ok(PreparedEntrypoint::Runtime(RuntimeEntrypoint {
                config_path: config,
                process: RuntimeProcess::Worker(worker),
            }))
        }
    }
}

fn worker_list_output(module: Option<&str>) -> Result<String, WorkerResolutionError> {
    let names = match module {
        None => modules::worker_module_names().collect::<Vec<_>>(),

        Some(module) => modules::worker_names(module)?.collect::<Vec<_>>(),
    };

    let mut output = names.join("\n");

    if !output.is_empty() {
        output.push('\n');
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{PreparedEntrypoint, RuntimeProcess, prepare};
    use crate::{
        cli::{Cli, Command, WorkerCommand},
        modules::WorkerResolutionError,
    };

    #[test]
    fn lists_and_resolves_the_canonical_worker_selection() {
        let module_list = prepare(worker_cli(WorkerCommand::List { module: None }))
            .expect("worker modules should resolve");
        let queue_worker_list = prepare(worker_cli(WorkerCommand::List {
            module: Some("queue".to_owned()),
        }))
        .expect("queue workers should resolve");

        assert_eq!(prepared_output(module_list), "queue\n");
        assert_eq!(
            prepared_output(queue_worker_list),
            "visibility-timeout-processor\n"
        );

        let runtime = prepare(Cli {
            config: Some("custom.yaml".into()),
            command: Command::Worker {
                command: WorkerCommand::Run {
                    module: "queue".to_owned(),
                    name: "visibility-timeout-processor".to_owned(),
                },
            },
        })
        .expect("known worker selection should resolve");

        let PreparedEntrypoint::Runtime(runtime) = runtime else {
            panic!("worker run should prepare a runtime entrypoint");
        };

        assert_eq!(runtime.config_path(), Some(Path::new("custom.yaml")));
        assert_eq!(runtime.process_mode(), "worker");
        assert_eq!(
            runtime.worker_selection(),
            Some(("queue", "visibility-timeout-processor"))
        );

        let RuntimeProcess::Worker(worker) = runtime.process else {
            panic!("worker run should prepare the selected worker");
        };
        assert_eq!(
            worker.into_registration().name,
            "visibility-timeout-processor"
        );
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
                && available == "visibility-timeout-processor"
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
}
