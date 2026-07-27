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
mod tests {
    use std::path::PathBuf;

    use clap::{Parser as _, error::ErrorKind};

    use super::{Cli, Command, WorkerCommand};

    #[test]
    fn parses_each_process_mode_and_worker_command() {
        let api = Cli::try_parse_from(["retsu", "api"]).expect("API command should parse");

        assert!(matches!(&api.command, Command::Api));

        let worker = Cli::try_parse_from([
            "retsu",
            "worker",
            "run",
            "queue",
            "visibility-timeout-processor",
        ])
        .expect("worker run command should parse");

        assert!(matches!(
            &worker.command,
            Command::Worker {
                command: WorkerCommand::Run { module, name },
            } if module.as_str() == "queue"
                && name.as_str() == "visibility-timeout-processor"
        ));

        let list = Cli::try_parse_from(["retsu", "worker", "list"])
            .expect("worker list command should parse");

        assert!(matches!(
            &list.command,
            Command::Worker {
                command: WorkerCommand::List { module: None },
            }
        ));

        let migrate =
            Cli::try_parse_from(["retsu", "migrate"]).expect("migration command should parse");

        assert!(matches!(&migrate.command, Command::Migrate));
    }

    #[test]
    fn requires_worker_subcommand_and_run_selection() {
        assert!(
            Cli::try_parse_from(["retsu", "worker"]).is_err(),
            "worker command should require list or run"
        );

        let missing_module = Cli::try_parse_from(["retsu", "worker", "run"])
            .expect_err("worker module should be required");

        assert_eq!(missing_module.kind(), ErrorKind::MissingRequiredArgument);

        let missing_name = Cli::try_parse_from(["retsu", "worker", "run", "queue"])
            .expect_err("worker name should be required");

        assert_eq!(missing_name.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn accepts_global_config_before_or_after_worker_selection() {
        let before = Cli::try_parse_from(["retsu", "--config", "custom.yaml", "api"])
            .expect("global option before command should parse");

        let after = Cli::try_parse_from([
            "retsu",
            "worker",
            "run",
            "queue",
            "visibility-timeout-processor",
            "--config",
            "custom.yaml",
        ])
        .expect("global option after worker selection should parse");

        assert!(matches!(&before.command, Command::Api));

        assert!(matches!(
            &after.command,
            Command::Worker {
                command: WorkerCommand::Run { module, name },
            } if module.as_str() == "queue"
                && name.as_str() == "visibility-timeout-processor"
        ));

        assert_eq!(before.config, Some(PathBuf::from("custom.yaml")));
        assert_eq!(after.config, Some(PathBuf::from("custom.yaml")));
    }
}
