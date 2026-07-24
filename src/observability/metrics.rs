use std::time::Duration;

use opentelemetry::{
    KeyValue,
    metrics::{Histogram, Meter, MeterProvider, UpDownCounter},
};
use opentelemetry_sdk::{Resource, error::OTelSdkError, metrics::SdkMeterProvider};
use prometheus::{Encoder, Registry, TextEncoder};

#[derive(Clone)]
pub(crate) struct Metrics {
    registry: Registry,
    http: HttpMetrics,
}

impl Metrics {
    pub(crate) fn http(&self) -> &HttpMetrics {
        &self.http
    }

    pub(crate) fn encode_prometheus(&self) -> Result<Vec<u8>, prometheus::Error> {
        let metric_families = self.registry.gather();
        let mut body = Vec::new();

        TextEncoder::new().encode(&metric_families, &mut body)?;

        Ok(body)
    }
}

#[derive(Clone)]
pub(crate) struct HttpMetrics {
    request_duration: Histogram<f64>,
    active_requests: UpDownCounter<i64>,
}

impl HttpMetrics {
    fn new(meter: &Meter) -> Self {
        let request_duration = meter
            .f64_histogram("http.server.request.duration")
            .with_description("Duration of HTTP server requests")
            .with_unit("s")
            .with_boundaries(vec![
                0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
            ])
            .build();

        let active_requests = meter
            .i64_up_down_counter("http.server.active_requests")
            .with_description("Number of active HTTP server requests")
            .with_unit("{request}")
            .build();

        Self {
            request_duration,
            active_requests,
        }
    }

    pub(crate) fn request_started(&self, method: String, scheme: String) -> ActiveRequest {
        let attributes = [
            KeyValue::new("http.request.method", method),
            KeyValue::new("url.scheme", scheme),
        ];

        self.active_requests.add(1, &attributes);

        ActiveRequest {
            active_requests: self.active_requests.clone(),
            attributes,
        }
    }

    pub(crate) fn request_finished(
        &self,
        method: String,
        scheme: String,
        protocol_version: Option<&'static str>,
        route: Option<String>,
        status_code: u16,
        duration: Duration,
    ) {
        let mut attributes = vec![
            KeyValue::new("http.request.method", method),
            KeyValue::new("url.scheme", scheme),
            KeyValue::new("http.response.status_code", i64::from(status_code)),
        ];

        if let Some(protocol_version) = protocol_version {
            attributes.push(KeyValue::new("network.protocol.version", protocol_version));
        }

        if let Some(route) = route {
            attributes.push(KeyValue::new("http.route", route));
        }

        if status_code >= 500 {
            attributes.push(KeyValue::new("error.type", status_code.to_string()));
        }

        self.request_duration
            .record(duration.as_secs_f64(), &attributes);
    }
}

pub(crate) struct ActiveRequest {
    active_requests: UpDownCounter<i64>,
    attributes: [KeyValue; 2],
}

impl Drop for ActiveRequest {
    fn drop(&mut self) {
        self.active_requests.add(-1, &self.attributes);
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

    let meter = provider.meter(env!("CARGO_PKG_NAME"));

    let metrics = Metrics {
        registry,
        http: HttpMetrics::new(&meter),
    };

    Ok((provider, metrics))
}
