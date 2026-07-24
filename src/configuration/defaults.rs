use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

use super::{
    AppConfiguration,
    schema::{Environment, HttpConfig, LogFormat, LoggingConfig},
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

fn logging_filter() -> String {
    "info".to_owned()
}

fn logging_format() -> LogFormat {
    LogFormat::Pretty
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_address: http_bind_address(),
            port: http_port(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: logging_filter(),
            format: logging_format(),
        }
    }
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            environment: environment(),
            http: HttpConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}
