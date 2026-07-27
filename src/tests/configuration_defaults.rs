use std::path::Path;

use super::{AppConfiguration, LogFormat};

#[test]
fn checked_in_yaml_matches_programmatic_defaults() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/retsu.yaml");
    let settings = config::Config::builder()
        .add_source(config::File::from(path).required(true))
        .build()
        .expect("checked-in configuration should load");
    let from_file = settings
        .try_deserialize::<AppConfiguration>()
        .expect("checked-in configuration should deserialize");
    let defaults = AppConfiguration::default();

    assert_eq!(from_file.environment, defaults.environment);
    assert_eq!(from_file.http.bind_address, defaults.http.bind_address);
    assert_eq!(from_file.http.port, defaults.http.port);
    assert_eq!(from_file.logging.filter, defaults.logging.filter);
    assert!(matches!(
        (from_file.logging.format, defaults.logging.format),
        (LogFormat::Pretty, LogFormat::Pretty) | (LogFormat::Json, LogFormat::Json)
    ));
    assert_eq!(
        from_file.telemetry.metrics.max_queues,
        defaults.telemetry.metrics.max_queues
    );
    assert_eq!(
        from_file.telemetry.traces.enabled,
        defaults.telemetry.traces.enabled
    );
    assert_eq!(
        from_file.telemetry.traces.filter,
        defaults.telemetry.traces.filter
    );
    assert_eq!(
        from_file.telemetry.traces.endpoint,
        defaults.telemetry.traces.endpoint
    );
    assert_eq!(
        from_file.telemetry.traces.timeout_seconds,
        defaults.telemetry.traces.timeout_seconds
    );
    assert_eq!(from_file.database.url, defaults.database.url);
    assert_eq!(
        from_file.database.max_connections,
        defaults.database.max_connections
    );
    assert_eq!(
        from_file.database.acquire_timeout_seconds,
        defaults.database.acquire_timeout_seconds
    );
    assert_eq!(
        from_file.worker.shutdown_timeout_seconds,
        defaults.worker.shutdown_timeout_seconds
    );
    assert_eq!(
        from_file.worker.management.bind_address,
        defaults.worker.management.bind_address
    );
    assert_eq!(
        from_file.worker.management.port,
        defaults.worker.management.port
    );
}
