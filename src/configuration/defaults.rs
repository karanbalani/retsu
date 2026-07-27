use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

use super::{
    AppConfiguration,
    schema::{
        DatabaseConfig, Environment, HttpConfig, LogFormat, LoggingConfig, MetricsConfig,
        TelemetryConfig, TraceExportConfig, WorkerConfig, WorkerManagementConfig,
    },
};

pub(super) fn default_config_path() -> PathBuf {
    PathBuf::from("config/retsu.yaml") // TODO: change this
}

fn environment() -> Environment {
    Environment::Local
}

fn http_bind_address() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn http_port() -> u16 {
    2424 // ee: spells to-and-fro
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_address: http_bind_address(),
            port: http_port(),
        }
    }
}

fn logging_filter() -> String {
    "info".to_owned()
}

fn logging_format() -> LogFormat {
    LogFormat::Pretty
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: logging_filter(),
            format: logging_format(),
        }
    }
}

fn metrics_max_queues() -> u32 {
    10_000
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            max_queues: metrics_max_queues(),
        }
    }
}

fn trace_export_enabled() -> bool {
    false
}

fn trace_export_filter() -> String {
    "info".to_owned()
}

fn trace_export_endpoint() -> String {
    "http://127.0.0.1:24241".to_owned()
}

fn trace_export_timeout_seconds() -> u64 {
    5
}

impl Default for TraceExportConfig {
    fn default() -> Self {
        Self {
            enabled: trace_export_enabled(),
            filter: trace_export_filter(),
            endpoint: trace_export_endpoint(),
            timeout_seconds: trace_export_timeout_seconds(),
        }
    }
}

fn database_url() -> String {
    "postgres://retsu:retsu_local@127.0.0.1:24240/retsu".to_owned()
}

fn database_max_connections() -> u32 {
    10
}

fn database_acquire_timeout_seconds() -> u64 {
    5
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: database_url(),
            max_connections: database_max_connections(),
            acquire_timeout_seconds: database_acquire_timeout_seconds(),
        }
    }
}

fn worker_shutdown_timeout_seconds() -> u64 {
    30
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_seconds: worker_shutdown_timeout_seconds(),
            management: WorkerManagementConfig::default(),
        }
    }
}

fn worker_management_bind_address() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn worker_management_port() -> u16 {
    24247
}

impl Default for WorkerManagementConfig {
    fn default() -> Self {
        Self {
            bind_address: worker_management_bind_address(),
            port: worker_management_port(),
        }
    }
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            environment: environment(),
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
            telemetry: TelemetryConfig::default(),
            database: DatabaseConfig::default(),
            worker: WorkerConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
