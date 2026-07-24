mod defaults;
mod error;
mod schema;

use std::path::Path;
use validator::Validate;

pub(crate) use error::ConfigurationError;
pub(crate) use schema::{AppConfiguration, LogFormat, LoggingConfig};

pub(crate) fn load(config_path: Option<&Path>) -> Result<AppConfiguration, ConfigurationError> {
    let builder = config::Config::builder();

    let builder = match config_path {
        Some(path) => builder.add_source(config::File::from(path).required(true)),
        None => {
            builder.add_source(config::File::from(defaults::default_config_path()).required(false))
        }
    };

    let settings = builder
        .add_source(
            config::Environment::with_prefix("RETSU")
                .prefix_separator("_")
                .separator("__")
                .try_parsing(true),
        )
        .build()?;

    let configuration = settings.try_deserialize::<AppConfiguration>()?;

    configuration.validate()?;

    Ok(configuration)
}
