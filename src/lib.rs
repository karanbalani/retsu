mod cli;
mod configuration;
mod entrypoints;
mod observability;

use clap::Parser;
use tracing::Instrument;

pub async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    let configuration = configuration::load(cli.config.as_deref())?;

    let observability = observability::initialize(&configuration)?;

    let process_mode = cli.command.as_str();

    let process_span = tracing::info_span!(
        "application",
        service_name = env!("CARGO_PKG_NAME"),
        service_version = env!("CARGO_PKG_VERSION"),
        environment = %configuration.environment,
        process_mode
    );

    let result = async move {
        tracing::info!("process mode started");

        let result = match cli.command {
            cli::Command::Api => entrypoints::api::run(configuration).await,

            cli::Command::Worker => entrypoints::worker::run(configuration).await,

            cli::Command::Migrate => entrypoints::migrate::run(configuration).await,
        };

        match &result {
            Ok(_) => {
                tracing::info!("process mode exited");
            }
            Err(error) => {
                tracing::error!(error = %error, "process mode failed");
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
