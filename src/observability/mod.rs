mod error;

use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

use crate::configuration::{LogFormat, LoggingConfig};

pub(crate) use error::ObservabilityError;

pub(crate) fn initialize(configuration: &LoggingConfig) -> Result<(), ObservabilityError> {
    let filter = EnvFilter::try_new(&configuration.filter)?;

    match configuration.format {
        LogFormat::Pretty => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .finish()
            .try_init()?,
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .json()
            .finish()
            .try_init()?,
    }

    Ok(())
}
