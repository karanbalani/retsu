use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ObservabilityError {
    #[error("invalid tracing filter: {0}")]
    InvalidFilter(#[from] tracing_subscriber::filter::ParseError),

    #[error("failed to install the global tracing subscriber: {0}")]
    Initialization(#[from] tracing_subscriber::util::TryInitError),
}
