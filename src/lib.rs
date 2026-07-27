mod api;
mod app;
mod cli;
mod configuration;
mod database;
mod entrypoints;
mod http;
mod management;
mod modules;
mod observability;
mod shutdown;
mod worker;

use std::io::Write as _;

use clap::Parser;
use tracing::Instrument;

pub async fn run() -> anyhow::Result<()> {
    let prepared = entrypoints::prepare(cli::Cli::parse())?;

    let runtime: entrypoints::RuntimeEntrypoint = match prepared {
        entrypoints::PreparedEntrypoint::Output(output) => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(output.as_bytes())?;
            return Ok(());
        }

        entrypoints::PreparedEntrypoint::Runtime(runtime) => runtime,
    };

    let configuration = configuration::load(runtime.config_path())?;

    let observability = observability::initialize(&configuration)?;
    let metrics = observability.metrics();

    let process_mode = runtime.process_mode();
    let worker_selection = runtime.worker_selection();

    let process_span = tracing::info_span!(
        "application",
        service_name = env!("CARGO_PKG_NAME"),
        service_version = env!("CARGO_PKG_VERSION"),
        environment = %configuration.environment,
        process_mode,
        worker.module = tracing::field::Empty,
        worker.name = tracing::field::Empty,
    );

    if let Some((module, name)) = worker_selection {
        process_span.record("worker.module", module);
        process_span.record("worker.name", name);
    }

    let result = async move {
        tracing::info!("process mode started");

        let result = runtime.run(configuration, metrics).await;

        match &result {
            Ok(()) => {
                tracing::info!("process mode exited");
            }

            Err(error) => {
                tracing::error!(
                    error = %error,
                    "process mode failed"
                );
            }
        }

        result
    }
    .instrument(process_span)
    .await;

    let shutdown_result = observability.shutdown();

    result?;
    shutdown_result?;

    Ok(())
}
