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
