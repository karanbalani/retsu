use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};

use super::CacheError;
use crate::configuration::DistributedCacheConfig;

pub(crate) trait RedisProtocolCommands: Clone + Send + Sync + 'static {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;

    async fn set(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), CacheError>;

    async fn set_if_absent(
        &self,
        key: &str,
        value: &[u8],
        ttl: Duration,
    ) -> Result<bool, CacheError>;
}

#[derive(Clone)]
pub(crate) struct RedisProtocolCache {
    connection: ConnectionManager,
}

impl RedisProtocolCache {
    pub(crate) fn new(configuration: &DistributedCacheConfig) -> Result<Self, CacheError> {
        let client = redis::Client::open(configuration.url.as_str()).map_err(CacheError::new)?;
        let connection_configuration = ConnectionManagerConfig::new()
            .set_number_of_retries(0)
            .set_connection_timeout(Some(Duration::from_millis(
                configuration.connection_timeout_milliseconds,
            )))
            .set_response_timeout(Some(Duration::from_millis(
                configuration.command_timeout_milliseconds,
            )));
        let connection = ConnectionManager::new_lazy_with_config(client, connection_configuration)
            .map_err(CacheError::new)?;

        Ok(Self { connection })
    }
}

impl RedisProtocolCommands for RedisProtocolCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut connection = self.connection.clone();

        redis::cmd("GET")
            .arg(key)
            .query_async(&mut connection)
            .await
            .map_err(CacheError::new)
    }

    async fn set(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), CacheError> {
        let mut connection = self.connection.clone();

        redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("PX")
            .arg(ttl_milliseconds(ttl))
            .query_async(&mut connection)
            .await
            .map_err(CacheError::new)
    }

    async fn set_if_absent(
        &self,
        key: &str,
        value: &[u8],
        ttl: Duration,
    ) -> Result<bool, CacheError> {
        let mut connection = self.connection.clone();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("NX")
            .arg("PX")
            .arg(ttl_milliseconds(ttl))
            .query_async(&mut connection)
            .await
            .map_err(CacheError::new)?;

        Ok(result.is_some())
    }
}

fn ttl_milliseconds(ttl: Duration) -> u64 {
    u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX)
}
