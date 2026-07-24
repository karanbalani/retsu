use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "retsu",
    version,
    about = "an observable, distributed priority queue"
)]
pub(crate) struct Cli {
    /// Load configuration from a YAML file
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the HTTP API server
    Api,

    /// Run the background workers
    Worker,

    /// Apply the pending database migrations
    Migrate,
}
