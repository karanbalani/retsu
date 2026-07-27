use opentelemetry::Key;

use crate::configuration::AppConfiguration;

use super::{build_resource, build_tracer_provider};

#[test]
fn resource_contains_stable_service_identity() {
    let configuration = AppConfiguration::default();

    let resource = build_resource(&configuration);

    assert_eq!(
        resource
            .get(&Key::new("service.name"))
            .map(|value| value.to_string()),
        Some(env!("CARGO_PKG_NAME").to_owned())
    );
    assert_eq!(
        resource
            .get(&Key::new("service.version"))
            .map(|value| value.to_string()),
        Some(env!("CARGO_PKG_VERSION").to_owned())
    );
    assert_eq!(
        resource
            .get(&Key::new("deployment.environment.name"))
            .map(|value| value.to_string()),
        Some("local".to_owned())
    );
}

#[test]
fn disabled_trace_export_does_not_build_an_exporter() {
    let configuration = AppConfiguration::default();
    let resource = build_resource(&configuration);

    let provider =
        build_tracer_provider(&configuration, resource).expect("disabled exporter should not fail");

    assert!(provider.is_none());
}
