use std::{hash::Hash, sync::Arc, time::Instant};

use moka::future::Cache as MokaCache;

use crate::observability::CacheMetrics;

use super::{Cache, CacheError};

#[derive(Clone, Copy)]
pub(crate) struct MemoryCachePolicy {
    max_entries: u64,
    max_capacity_bytes: u64,
}

impl MemoryCachePolicy {
    pub(crate) fn new(max_entries: u64, max_capacity_bytes: u64) -> Self {
        assert!(max_entries > 0, "cache max entries must be positive");
        assert!(
            max_capacity_bytes > 0,
            "cache byte capacity must be positive"
        );

        Self {
            max_entries,
            max_capacity_bytes,
        }
    }
}

pub(crate) struct MemoryCache<K, V> {
    name: &'static str,
    inner: MokaCache<K, Arc<V>>,
    metrics: CacheMetrics,
}

impl<K, V> Clone for MemoryCache<K, V> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            inner: self.inner.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

enum LoadFailure<E> {
    NotFound,
    Source(Arc<E>),
}

impl<K, V> MemoryCache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    pub(crate) fn new<W>(
        name: &'static str,
        policy: MemoryCachePolicy,
        weigher: W,
        metrics: CacheMetrics,
    ) -> Self
    where
        W: Fn(&K, &V) -> u32 + Send + Sync + 'static,
    {
        let minimum_entry_weight = policy
            .max_capacity_bytes
            .div_ceil(policy.max_entries)
            .clamp(1, u64::from(u32::MAX)) as u32;

        let inner = MokaCache::builder()
            .name(name)
            .max_capacity(policy.max_capacity_bytes)
            .weigher(move |key, value: &Arc<V>| {
                weigher(key, value.as_ref()).max(minimum_entry_weight)
            })
            .build();

        Self {
            name,
            inner,
            metrics,
        }
    }

    #[cfg(test)]
    pub(super) async fn run_pending_tasks(&self) {
        self.inner.run_pending_tasks().await;
    }

    #[cfg(test)]
    pub(super) fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    #[cfg(test)]
    pub(super) fn weighted_size(&self) -> u64 {
        self.inner.weighted_size()
    }
}

impl<K, V> Cache<K, V> for MemoryCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    async fn get_or_load<E, F, Fut>(&self, key: K, loader: F) -> Result<Option<Arc<V>>, Arc<E>>
    where
        E: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, E>>,
    {
        if let Some(value) = self.inner.get(&key).await {
            self.metrics.request(self.name, "hit");
            return Ok(Some(value));
        }

        self.metrics.request(self.name, "miss");

        let metrics = self.metrics.clone();
        let name = self.name;
        let result = self
            .inner
            .try_get_with(key, async move {
                let started = Instant::now();
                let result = loader().await;

                match result {
                    Ok(Some(value)) => {
                        metrics.load_finished(name, started.elapsed(), "success");
                        Ok(Arc::new(value))
                    }
                    Ok(None) => {
                        metrics.load_finished(name, started.elapsed(), "not_found");
                        Err(LoadFailure::NotFound)
                    }
                    Err(error) => {
                        metrics.load_finished(name, started.elapsed(), "error");
                        Err(LoadFailure::Source(Arc::new(error)))
                    }
                }
            })
            .await;

        match result {
            Ok(value) => Ok(Some(value)),
            Err(error) => match error.as_ref() {
                LoadFailure::NotFound => Ok(None),
                LoadFailure::Source(error) => Err(Arc::clone(error)),
            },
        }
    }

    async fn insert(&self, key: K, value: Arc<V>) -> Result<(), CacheError> {
        self.inner.insert(key, value).await;
        Ok(())
    }

    async fn invalidate(&self, key: &K) -> Result<(), CacheError> {
        self.inner.invalidate(key).await;
        Ok(())
    }
}
