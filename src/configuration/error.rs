use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ConfigurationError {
    #[error("failed to load configuration: {0}")]
    Load(#[from] config::ConfigError),

    #[error("configuration validation failed: {0}")]
    Validation(#[from] validator::ValidationErrors),
}
