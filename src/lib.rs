mod cli;
mod entrypoints;

use clap::Parser;

pub async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Api => entrypoints::api::run().await,
        cli::Command::Worker => entrypoints::worker::run().await,
        cli::Command::Migrate => entrypoints::migrate::run().await,
    }
}
