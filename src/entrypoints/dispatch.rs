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
