use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

use super::{
    AppConfiguration,
    schema::{
        CacheConfig, CachePolicyConfig, DatabaseConfig, Environment, HttpConfig, LogFormat,
        LoggingConfig, MetricsConfig, TelemetryConfig, TraceExportConfig, WorkerConfig,
        WorkerManagementConfig,
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

fn cache_max_entries() -> u64 {
    10_000
}

fn cache_max_capacity_bytes() -> u64 {
    8 * 1024 * 1024
}

impl Default for CachePolicyConfig {
    fn default() -> Self {
        Self {
            max_entries: cache_max_entries(),
            max_capacity_bytes: cache_max_capacity_bytes(),
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
            cache: CacheConfig::default(),
            database: DatabaseConfig::default(),
            worker: WorkerConfig::default(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/configuration_defaults.rs"]
mod tests;
