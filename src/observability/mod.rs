mod error;
mod metrics;

use std::time::Duration;

use opentelemetry::{KeyValue, global, trace::TracerProvider as _};

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};

use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::configuration::{AppConfiguration, LogFormat};

pub(crate) use error::ObservabilityError;
pub(crate) use metrics::Metrics;

pub(crate) struct Observability {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: SdkMeterProvider,
    metrics: Metrics,
}

impl Observability {
    pub(crate) fn metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    pub(crate) fn shutdown(self) -> Result<(), ObservabilityError> {
        let metrics_result = self
            .meter_provider
            .shutdown()
            .map_err(ObservabilityError::MetricsShutdown);

        let traces_result = self
            .tracer_provider
            .map(|provider| {
                provider
                    .shutdown()
                    .map_err(ObservabilityError::TraceShutdown)
            })
            .transpose();

        metrics_result?;
        traces_result?;

        Ok(())
    }
}

pub(crate) fn initialize(
    configuration: &AppConfiguration,
) -> Result<Observability, ObservabilityError> {
    let resource = build_resource(configuration);

    let tracer_provider = build_tracer_provider(configuration, resource.clone())?;

    let (meter_provider, metrics) =
        metrics::initialize(resource).map_err(ObservabilityError::MetricsExporter)?;

    install_subscriber(configuration, tracer_provider.as_ref())?;

    if let Some(provider) = &tracer_provider {
        global::set_text_map_propagator(TraceContextPropagator::new());
        global::set_tracer_provider(provider.clone());
    }

    global::set_meter_provider(meter_provider.clone());

    Ok(Observability {
        tracer_provider,
        meter_provider,
        metrics,
    })
}

fn build_resource(configuration: &AppConfiguration) -> Resource {
    Resource::builder()
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_attributes([
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new(
                "deployment.environment.name",
                configuration.environment.to_string(),
            ),
        ])
        .build()
}

fn build_tracer_provider(
    configuration: &AppConfiguration,
    resource: Resource,
) -> Result<Option<SdkTracerProvider>, ObservabilityError> {
    let trace_configuration = &configuration.telemetry.traces;

    if !trace_configuration.enabled {
        return Ok(None);
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(trace_configuration.endpoint.clone())
        .with_timeout(Duration::from_secs(trace_configuration.timeout_seconds))
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();

    Ok(Some(provider))
}

fn install_subscriber(
    configuration: &AppConfiguration,
    tracer_provider: Option<&SdkTracerProvider>,
) -> Result<(), ObservabilityError> {
    let logging_filter = EnvFilter::try_new(&configuration.logging.filter)
        .map_err(ObservabilityError::InvalidLoggingFilter)?;

    let trace_filter = EnvFilter::try_new(&configuration.telemetry.traces.filter)
        .map_err(ObservabilityError::InvalidTraceFilter)?;

    match (configuration.logging.format, tracer_provider) {
        (LogFormat::Pretty, None) => {
            let formatting_layer = fmt::layer().with_target(true).with_filter(logging_filter);

            tracing_subscriber::registry()
                .with(formatting_layer)
                .try_init()?;
        }

        (LogFormat::Json, None) => {
            let formatting_layer = fmt::layer()
                .with_target(true)
                .json()
                .with_filter(logging_filter);

            tracing_subscriber::registry()
                .with(formatting_layer)
                .try_init()?;
        }

        (LogFormat::Pretty, Some(provider)) => {
            let tracer = provider.tracer(env!("CARGO_PKG_NAME"));
            let formatting_layer = fmt::layer().with_target(true).with_filter(logging_filter);
            let telemetry_layer = tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(trace_filter);

            tracing_subscriber::registry()
                .with(formatting_layer)
                .with(telemetry_layer)
                .try_init()?;
        }

        (LogFormat::Json, Some(provider)) => {
            let tracer = provider.tracer(env!("CARGO_PKG_NAME"));
            let formatting_layer = fmt::layer()
                .with_target(true)
                .json()
                .with_filter(logging_filter);
            let telemetry_layer = tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(trace_filter);

            tracing_subscriber::registry()
                .with(formatting_layer)
                .with(telemetry_layer)
                .try_init()?;
        }
    }

    Ok(())
}
