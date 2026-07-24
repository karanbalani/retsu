use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ObservabilityError {
    #[error("invalid logging filter: {0}")]
    InvalidLoggingFilter(tracing_subscriber::filter::ParseError),

    #[error("invalid trace filter: {0}")]
    InvalidTraceFilter(tracing_subscriber::filter::ParseError),

    #[error("failed to install the global tracing subscriber: {0}")]
    Initialization(#[from] tracing_subscriber::util::TryInitError),

    #[error("failed to build the OTLP trace exporter: {0}")]
    Exporter(#[from] opentelemetry_otlp::ExporterBuildError),

    #[error("failed to shut down the trace provider: {0}")]
    Shutdown(#[from] opentelemetry_sdk::error::OTelSdkError),
}
