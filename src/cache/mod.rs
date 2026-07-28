mod memory;

use std::{future::Future, sync::Arc};

pub(crate) use memory::{MemoryCache, MemoryCachePolicy};

#[derive(Debug, thiserror::Error)]
#[error("cache backend operation failed")]
pub(crate) struct CacheError {
    #[source]
    source: anyhow::Error,
}

pub(crate) trait Cache<K, V>: Clone {
    async fn get_or_load<E, F, Fut>(&self, key: K, loader: F) -> Result<Option<Arc<V>>, Arc<E>>
    where
        E: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>;

    async fn insert(&self, key: K, value: Arc<V>) -> Result<(), CacheError>;

    async fn invalidate(&self, key: &K) -> Result<(), CacheError>;
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
