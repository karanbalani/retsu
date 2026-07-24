use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

use super::{
    AppConfiguration,
    schema::{
        DatabaseConfig, Environment, HttpConfig, LogFormat, LoggingConfig, TelemetryConfig,
        TraceExportConfig,
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

fn trace_export_enabled() -> bool {
    false
}

fn trace_export_filter() -> String {
    "info".to_owned()
}

fn trace_export_endpoint() -> String {
    "http://127.0.0.1:4317".to_owned()
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
    "postgres://retsu@127.0.0.1:5432/retsu".to_owned()
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

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            environment: environment(),
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
            telemetry: TelemetryConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}
