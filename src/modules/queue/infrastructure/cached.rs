use std::sync::Arc;

use anyhow::anyhow;
use uuid::Uuid;

use crate::cache::Cache;

use super::super::{
    application::{CreateQueueOutcome, QueueRepository},
    domain::{Queue, QueueDetails},
};

#[derive(Clone)]
pub(in crate::modules::queue) struct CachedQueueRepository<R, C> {
    source: R,
    cache: C,
}

impl<R, C> CachedQueueRepository<R, C> {
    pub(in crate::modules::queue) fn new(source: R, cache: C) -> Self {
        Self { source, cache }
    }
}

impl<R, C> CachedQueueRepository<R, C>
where
    C: Cache<Uuid, QueueDetails>,
{
    pub(in crate::modules::queue) async fn invalidate_queue_details(&self, queue_id: Uuid) {
        if let Err(error) = self.cache.invalidate(&queue_id).await {
            tracing::warn!(
                queue.id = %queue_id,
                error = &error as &dyn std::error::Error,
                "failed to invalidate queue details cache"
            );
        }
    }
}

impl<R, C> QueueRepository for CachedQueueRepository<R, C>
where
    R: QueueRepository + Clone,
    C: Cache<Uuid, QueueDetails>,
{
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        let outcome = self.source.create_queue(queue).await?;

        if matches!(outcome, CreateQueueOutcome::Created) {
            let details = queue.details();
            let queue_id = details.id();

            if let Err(error) = self.cache.insert(queue_id, Arc::new(details)).await {
                tracing::warn!(
                    queue.id = %queue_id,
                    error = &error as &dyn std::error::Error,
                    "failed to populate queue details cache after queue creation"
                );
            }
        }

        Ok(outcome)
    }

    async fn queue_details(&self, queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error> {
        self.cache
            .get_or_load(queue_id, || self.source.queue_details(queue_id))
            .await
            .map(|details| details.map(|details| details.as_ref().clone()))
            .map_err(|error| anyhow!("failed to load queue details: {error:#}"))
    }
}

#[cfg(test)]
#[path = "../tests/infrastructure_cached_repository.rs"]
mod tests;
