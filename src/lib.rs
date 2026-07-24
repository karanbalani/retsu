mod cli;
mod configuration;
mod entrypoints;

use clap::Parser;

pub async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    let configuration = configuration::load(cli.config.as_deref())?;

    match cli.command {
        cli::Command::Api => entrypoints::api::run(configuration).await,
        cli::Command::Worker => entrypoints::worker::run(configuration).await,
        cli::Command::Migrate => entrypoints::migrate::run(configuration).await,
    }
}
