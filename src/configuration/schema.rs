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
}

impl WorkerConfig {
    pub(crate) fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_seconds)
    }
}

#[cfg(test)]
mod tests {
    use validator::Validate as _;

    use super::AppConfiguration;

    fn assert_invalid(mutate: impl FnOnce(&mut AppConfiguration)) {
        let mut configuration = AppConfiguration::default();
        mutate(&mut configuration);

        assert!(
            configuration.validate().is_err(),
            "configuration should be rejected"
        );
    }

    #[test]
    fn accepts_all_validation_boundaries() {
        let mut configuration = AppConfiguration::default();
        configuration.http.port = 1;
        configuration.telemetry.metrics.max_queues = 1;
        configuration.telemetry.traces.timeout_seconds = 1;
        configuration.database.max_connections = 1;
        configuration.database.acquire_timeout_seconds = 5;
        configuration.worker.shutdown_timeout_seconds = 1;
        configuration.worker.management.port = 1;

        configuration
            .validate()
            .expect("minimum boundaries should be valid");

        configuration.telemetry.metrics.max_queues = 100_000;
        configuration.telemetry.traces.timeout_seconds = 60;
        configuration.database.acquire_timeout_seconds = 60;
        configuration.worker.shutdown_timeout_seconds = 300;

        configuration
            .validate()
            .expect("maximum boundaries should be valid");
    }

    #[test]
    fn rejects_values_outside_the_validation_contract() {
        assert_invalid(|configuration| configuration.http.port = 0);
        assert_invalid(|configuration| configuration.logging.filter.clear());
        assert_invalid(|configuration| configuration.telemetry.metrics.max_queues = 0);
        assert_invalid(|configuration| configuration.telemetry.metrics.max_queues = 100_001);
        assert_invalid(|configuration| configuration.telemetry.traces.filter.clear());
        assert_invalid(|configuration| {
            configuration.telemetry.traces.endpoint = "not a URL".to_owned();
        });
        assert_invalid(|configuration| configuration.telemetry.traces.timeout_seconds = 0);
        assert_invalid(|configuration| configuration.telemetry.traces.timeout_seconds = 61);
        assert_invalid(|configuration| {
            configuration.database.url = "not a URL".to_owned();
        });
        assert_invalid(|configuration| configuration.database.max_connections = 0);
        assert_invalid(|configuration| configuration.database.acquire_timeout_seconds = 4);
        assert_invalid(|configuration| configuration.database.acquire_timeout_seconds = 61);
        assert_invalid(|configuration| configuration.worker.shutdown_timeout_seconds = 0);
        assert_invalid(|configuration| configuration.worker.shutdown_timeout_seconds = 301);
        assert_invalid(|configuration| configuration.worker.management.port = 0);
    }
}
