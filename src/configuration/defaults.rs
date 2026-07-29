use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

use super::{
    AppConfiguration,
    schema::{
        CacheConfig, CachePolicyConfig, DatabaseConfig, DeadLetterMessageCleanerConfig,
        DistributedCacheConfig, Environment, ExpiredMessageCleanerConfig, HttpConfig,
        InMemoryCacheConfig, LogFormat, LoggingConfig, MetricsConfig, QueueWorkerConfig,
        StateMetricsCollectorConfig, TelemetryConfig, TraceExportConfig, WorkerConfig,
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

fn cache_enabled() -> bool {
    true
}

impl Default for InMemoryCacheConfig {
    fn default() -> Self {
        Self {
            enabled: cache_enabled(),
            regions: Default::default(),
        }
    }
}

impl Default for CachePolicyConfig {
    fn default() -> Self {
        Self {
            max_entries: cache_max_entries(),
            max_capacity_bytes: cache_max_capacity_bytes(),
        }
    }
}

fn distributed_cache_url() -> String {
    "redis://127.0.0.1:24251".to_owned()
}

fn distributed_cache_connection_timeout_milliseconds() -> u64 {
    500
}

fn distributed_cache_command_timeout_milliseconds() -> u64 {
    20
}

impl Default for DistributedCacheConfig {
    fn default() -> Self {
        Self {
            enabled: cache_enabled(),
            url: distributed_cache_url(),
            connection_timeout_milliseconds: distributed_cache_connection_timeout_milliseconds(),
            command_timeout_milliseconds: distributed_cache_command_timeout_milliseconds(),
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
            queue: QueueWorkerConfig::default(),
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

fn dead_letter_message_cleaner_retention_seconds() -> u64 {
    1_209_600
}

fn dead_letter_message_cleaner_processing_interval_seconds() -> u64 {
    60
}

fn dead_letter_message_cleaner_batch_size() -> u32 {
    500
}

fn dead_letter_message_cleaner_saturated_batch_delay_milliseconds() -> u64 {
    50
}

impl Default for DeadLetterMessageCleanerConfig {
    fn default() -> Self {
        Self {
            retention_seconds: dead_letter_message_cleaner_retention_seconds(),
            processing_interval_seconds: dead_letter_message_cleaner_processing_interval_seconds(),
            batch_size: dead_letter_message_cleaner_batch_size(),
            saturated_batch_delay_milliseconds:
                dead_letter_message_cleaner_saturated_batch_delay_milliseconds(),
        }
    }
}

fn expired_message_cleaner_processing_interval_seconds() -> u64 {
    60
}

fn expired_message_cleaner_batch_size() -> u32 {
    500
}

fn expired_message_cleaner_saturated_batch_delay_milliseconds() -> u64 {
    50
}

impl Default for ExpiredMessageCleanerConfig {
    fn default() -> Self {
        Self {
            processing_interval_seconds: expired_message_cleaner_processing_interval_seconds(),
            batch_size: expired_message_cleaner_batch_size(),
            saturated_batch_delay_milliseconds:
                expired_message_cleaner_saturated_batch_delay_milliseconds(),
        }
    }
}

fn state_metrics_collector_collection_interval_seconds() -> u64 {
    15
}

fn state_metrics_collector_leadership_retry_interval_seconds() -> u64 {
    15
}

impl Default for StateMetricsCollectorConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: state_metrics_collector_collection_interval_seconds(),
            leadership_retry_interval_seconds:
                state_metrics_collector_leadership_retry_interval_seconds(),
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
