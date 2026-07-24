mod cli;

use clap::Parser;

pub async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Api => println!("api mode"),
        cli::Command::Worker => println!("worker mode"),
        cli::Command::Migrate => println!("migrate mode"),
    }

    Ok(())
}
