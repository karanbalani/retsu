mod defaults;
mod error;
mod schema;

use std::path::Path;
use validator::Validate;

pub(crate) use error::ConfigurationError;
pub(crate) use schema::{
    AppConfiguration, DatabaseConfig, DeadLetterMessageCleanerConfig, DistributedCacheConfig,
    ExpiredMessageCleanerConfig, InMemoryCacheConfig, LogFormat, StateMetricsCollectorConfig,
    WorkerConfig,
};

pub(crate) fn load(config_path: Option<&Path>) -> Result<AppConfiguration, ConfigurationError> {
    load_with_environment(config_path, environment_source())
}

fn load_with_environment(
    config_path: Option<&Path>,
    environment: config::Environment,
) -> Result<AppConfiguration, ConfigurationError> {
    let builder = config::Config::builder();

    let builder = match config_path {
        Some(path) => builder.add_source(config::File::from(path).required(true)),
        None => {
            builder.add_source(config::File::from(defaults::default_config_path()).required(false))
        }
    };

    let settings = builder.add_source(environment).build()?;

    let configuration = settings.try_deserialize::<AppConfiguration>()?;

    configuration.validate()?;

    Ok(configuration)
}

fn environment_source() -> config::Environment {
    configure_environment(config::Environment::with_prefix("RETSU"))
}

fn configure_environment(environment: config::Environment) -> config::Environment {
    environment
        .prefix_separator("_")
        .separator("__")
        .try_parsing(true)
}

#[cfg(test)]
#[path = "../tests/configuration.rs"]
mod tests;
