use std::{
    fmt::Display,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AppConfiguration {
    pub(crate) environment: Environment,

    #[validate(nested)]
    pub(crate) http: HttpConfig,

    #[validate(nested)]
    pub(crate) logging: LoggingConfig,

    #[validate(nested)]
    pub(crate) telemetry: TelemetryConfig,

    #[validate(nested)]
    pub(crate) cache: CacheConfig,

    #[validate(nested)]
    pub(crate) database: DatabaseConfig,

    #[validate(nested)]
    pub(crate) worker: WorkerConfig,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Environment {
    #[default]
    Local,
    Test,
    Staging,
    Production,
}

impl Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Local => "local",
            Self::Test => "test",
            Self::Staging => "staging",
            Self::Production => "production",
        };

        f.write_str(value)
    }
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct HttpConfig {
    pub(crate) bind_address: IpAddr,

    #[validate(range(min = 1))]
    pub(crate) port: u16,
}

impl HttpConfig {
    pub(crate) fn socket_address(&self) -> SocketAddr {
        SocketAddr::new(self.bind_address, self.port)
    }
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LoggingConfig {
    #[validate(length(min = 1))]
    pub(crate) filter: String,

    pub(crate) format: LogFormat,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Default, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TelemetryConfig {
    #[validate(nested)]
    pub(crate) metrics: MetricsConfig,

    #[validate(nested)]
    pub(crate) traces: TraceExportConfig,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct MetricsConfig {
    #[validate(range(min = 1, max = 100_000))]
    pub(crate) max_queues: u32,
}

#[derive(Default, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CacheConfig {
    #[validate(nested)]
    pub(crate) in_memory: InMemoryCacheConfig,

    #[validate(nested)]
    pub(crate) distributed: DistributedCacheConfig,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct InMemoryCacheConfig {
    pub(crate) enabled: bool,

    #[validate(nested)]
    pub(crate) regions: InMemoryCacheRegionsConfig,
}

#[derive(Default, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct InMemoryCacheRegionsConfig {
    #[validate(nested)]
    pub(crate) queue_names: CachePolicyConfig,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CachePolicyConfig {
    #[validate(range(min = 1, max = 1_000_000))]
    pub(crate) max_entries: u64,

    #[validate(range(min = 1, max = 4_294_967_295_u64))] // 4GB
    pub(crate) max_capacity_bytes: u64,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DistributedCacheConfig {
    pub(crate) enabled: bool,

    #[validate(url)]
    pub(crate) url: String,

    #[validate(range(min = 1, max = 10_000))]
    pub(crate) connection_timeout_milliseconds: u64,

    #[validate(range(min = 1, max = 10_000))]
    pub(crate) command_timeout_milliseconds: u64,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TraceExportConfig {
    pub(crate) enabled: bool,

    #[validate(length(min = 1))]
    pub(crate) filter: String,

    #[validate(url)]
    pub(crate) endpoint: String,

    #[validate(range(min = 1, max = 60))]
    pub(crate) timeout_seconds: u64,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DatabaseConfig {
    #[validate(url)]
    pub(crate) url: String,

    #[validate(range(min = 1))]
    pub(crate) max_connections: u32,

    #[validate(range(min = 5, max = 60))]
    pub(crate) acquire_timeout_seconds: u64,
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct WorkerManagementConfig {
    pub(crate) bind_address: IpAddr,

    #[validate(range(min = 1))]
    pub(crate) port: u16,
}

impl WorkerManagementConfig {
    pub(crate) fn socket_address(&self) -> SocketAddr {
        SocketAddr::new(self.bind_address, self.port)
    }
}

#[derive(Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct WorkerConfig {
    #[validate(range(min = 1, max = 300))]
    pub(crate) shutdown_timeout_seconds: u64,

    #[validate(nested)]
    pub(crate) management: WorkerManagementConfig,

    #[validate(nested)]
    pub(crate) queue: QueueWorkerConfig,
}

impl WorkerConfig {
    pub(crate) fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_seconds)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct QueueWorkerConfig {
    #[validate(nested)]
    pub(crate) expired_message_cleaner: ExpiredMessageCleanerConfig,

    #[validate(nested)]
    pub(crate) state_metrics_collector: StateMetricsCollectorConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ExpiredMessageCleanerConfig {
    #[validate(range(min = 5, max = 3_600))]
    pub(crate) processing_interval_seconds: u64,

    #[validate(range(min = 1, max = 10_000))]
    pub(crate) batch_size: u32,

    #[validate(range(min = 1, max = 5_000))]
    pub(crate) saturated_batch_delay_milliseconds: u64,
}

impl ExpiredMessageCleanerConfig {
    pub(crate) fn processing_interval(&self) -> Duration {
        Duration::from_secs(self.processing_interval_seconds)
    }

    pub(crate) fn saturated_batch_delay(&self) -> Duration {
        Duration::from_millis(self.saturated_batch_delay_milliseconds)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Validate)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct StateMetricsCollectorConfig {
    #[validate(range(min = 5, max = 3_600))]
    pub(crate) collection_interval_seconds: u64,

    #[validate(range(min = 5, max = 300))]
    pub(crate) leadership_retry_interval_seconds: u64,
}

impl StateMetricsCollectorConfig {
    pub(crate) fn collection_interval(&self) -> Duration {
        Duration::from_secs(self.collection_interval_seconds)
    }

    pub(crate) fn leadership_retry_interval(&self) -> Duration {
        Duration::from_secs(self.leadership_retry_interval_seconds)
    }
}

#[cfg(test)]
#[path = "../tests/configuration_schema.rs"]
mod tests;
