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
pub(crate) enum Command {
    /// Run the HTTP API server
    Api,

    /// Inspect or run background workers
    Worker {
        #[command(subcommand)]
        command: WorkerCommand,
    },

    /// Apply the pending database migrations
    Migrate,
}

#[derive(Debug, Subcommand)]
pub(crate) enum WorkerCommand {
    /// List worker modules or workers owned by one module
    List {
        /// Module whose workers should be listed
        #[arg(value_name = "MODULE")]
        module: Option<String>,
    },

    /// Run one named worker
    Run {
        /// Module that owns the worker
        #[arg(value_name = "MODULE")]
        module: String,

        /// Worker name within the module
        #[arg(value_name = "NAME")]
        name: String,
    },
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
