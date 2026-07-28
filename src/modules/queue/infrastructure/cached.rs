use std::sync::Arc;

use anyhow::anyhow;
use uuid::Uuid;

use crate::cache::Cache;

use super::super::{
    application::{CreateQueueOutcome, QueueRepository},
    domain::Queue,
};

#[derive(Clone)]
pub(in crate::modules::queue) struct QueueNameCachingRepository<R, C> {
    source: R,
    cache: C,
}

impl<R, C> QueueNameCachingRepository<R, C> {
    pub(in crate::modules::queue) fn new(source: R, cache: C) -> Self {
        Self { source, cache }
    }
}

impl<R, C> QueueNameCachingRepository<R, C>
where
    C: Cache<Uuid, String>,
{
    pub(in crate::modules::queue) async fn invalidate_queue_name(&self, queue_id: Uuid) {
        if let Err(error) = self.cache.invalidate(&queue_id).await {
            tracing::warn!(
                queue.id = %queue_id,
                error = &error as &dyn std::error::Error,
                "failed to invalidate queue name cache"
            );
        }
    }
}

impl<R, C> QueueRepository for QueueNameCachingRepository<R, C>
where
    R: QueueRepository + Clone,
    C: Cache<Uuid, String>,
{
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        let outcome = self.source.create_queue(queue).await?;

        if matches!(outcome, CreateQueueOutcome::Created)
            && let Err(error) = self
                .cache
                .insert(queue.id(), Arc::new(queue.name().to_owned()))
                .await
        {
            tracing::warn!(
                queue.id = %queue.id(),
                error = &error as &dyn std::error::Error,
                "failed to populate queue name cache after queue creation"
            );
        }

        Ok(outcome)
    }

    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        self.cache
            .get_or_load(queue_id, || self.source.queue_name(queue_id))
            .await
            .map(|name| name.map(|name| name.as_ref().clone()))
            .map_err(|error| anyhow!("failed to load queue name: {error:#}"))
    }
}

#[cfg(test)]
#[path = "../tests/infrastructure_cached_repository.rs"]
mod tests;
