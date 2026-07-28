mod memory;
mod redis_protocol;

use std::{future::Future, sync::Arc};

pub(crate) use memory::{MemoryCache, MemoryCachePolicy};
pub(crate) use redis_protocol::{RedisProtocolCache, RedisProtocolCommands};

#[derive(Debug, thiserror::Error)]
#[error("cache backend operation failed")]
pub(crate) struct CacheError {
    #[source]
    source: anyhow::Error,
}

impl CacheError {
    pub(crate) fn new(source: impl Into<anyhow::Error>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

pub(crate) trait Cache<K, V>: Clone {
    async fn get_or_load<E, F, Fut>(&self, key: K, loader: F) -> Result<Option<Arc<V>>, Arc<E>>
    where
        E: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>;

    async fn insert(&self, key: K, value: Arc<V>) -> Result<(), CacheError>;
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
