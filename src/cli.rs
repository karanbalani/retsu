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

    /// Run one named background worker
    Worker {
        /// Module that owns the worker
        #[arg(value_name = "MODULE")]
        module: String,

        /// Worker name within the module
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Apply the pending database migrations
    Migrate,
}

impl Command {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Worker { .. } => "worker",
            Self::Migrate => "migrate",
        }
    }

    pub(crate) fn worker_selection(&self) -> Option<(&str, &str)> {
        match self {
            Self::Worker { module, name } => Some((module.as_str(), name.as_str())),
            Self::Api | Self::Migrate => None,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use std::path::PathBuf;

//     use clap::Parser as _;

//     use super::{Cli, Command};

//     #[test]
//     fn parses_each_process_mode() {
//         let cases = [("api", "api"), ("worker", "worker"), ("migrate", "migrate")];

//         for (argument, expected_mode) in cases {
//             let cli = Cli::try_parse_from(["retsu", argument]).expect("command should parse");

//             assert_eq!(cli.command.as_str(), expected_mode);
//             assert_eq!(cli.config, None);
//         }
//     }

//     #[test]
//     fn accepts_global_config_before_or_after_the_subcommand() {
//         let before = Cli::try_parse_from(["retsu", "--config", "custom.yaml", "api"])
//             .expect("global option before subcommand should parse");
//         let after = Cli::try_parse_from(["retsu", "worker", "--config", "custom.yaml"])
//             .expect("global option after subcommand should parse");

//         assert!(matches!(before.command, Command::Api));
//         assert!(matches!(after.command, Command::Worker { .. }));
//         assert_eq!(before.config, Some(PathBuf::from("custom.yaml")));
//         assert_eq!(after.config, Some(PathBuf::from("custom.yaml")));
//     }
// }
