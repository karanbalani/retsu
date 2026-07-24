use std::{
    fmt::Display,
    net::{IpAddr, SocketAddr},
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
    pub(crate) traces: TraceExportConfig,
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
