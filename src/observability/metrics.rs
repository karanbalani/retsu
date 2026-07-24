use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::{Resource, error::OTelSdkError, metrics::SdkMeterProvider};
use prometheus::{Encoder, Registry, TextEncoder};

#[derive(Clone)]
pub(crate) struct Metrics {
    registry: Registry,
}

impl Metrics {
    pub(crate) fn encode_prometheus(&self) -> Result<Vec<u8>, prometheus::Error> {
        let metric_families = self.registry.gather();
        let mut body = Vec::new();

        TextEncoder::new().encode(&metric_families, &mut body)?;

        Ok(body)
    }
}

pub(super) fn initialize(resource: Resource) -> Result<(SdkMeterProvider, Metrics), OTelSdkError> {
    let registry = Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()?;

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(exporter)
        .build();

    let _meter = provider.meter(env!("CARGO_PKG_NAME"));

    Ok((provider, Metrics { registry }))
}
