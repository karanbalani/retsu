use std::sync::Arc;

use anyhow::anyhow;
use uuid::Uuid;

use crate::cache::{Cache, MemoryCache};

use super::super::{
    application::{
        AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome,
        ExpiredMessagesCleanupSummary, QueueRepository,
    },
    domain::{Message, Queue, QueueConfigurationUpdate, QueueDetails},
};

#[derive(Clone)]
pub(in crate::modules::queue) struct L1QueueRepository<R> {
    source: R,
    queue_names: Option<MemoryCache<Uuid, String>>,
}

impl<R> L1QueueRepository<R> {
    pub(in crate::modules::queue) fn new(
        source: R,
        queue_names: Option<MemoryCache<Uuid, String>>,
    ) -> Self {
        Self {
            source,
            queue_names,
        }
    }
}

impl<R> QueueRepository for L1QueueRepository<R>
where
    R: QueueRepository,
{
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        let outcome = self.source.create_queue(queue).await?;

        if matches!(outcome, CreateQueueOutcome::Created)
            && let Some(queue_names) = &self.queue_names
            && let Err(error) = queue_names
                .insert(queue.id(), Arc::new(queue.name().to_owned()))
                .await
        {
            tracing::warn!(
                queue.id = %queue.id(),
                error = &error as &dyn std::error::Error,
                "failed to write through in-memory queue-name cache"
            );
        }

        Ok(outcome)
    }

    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        let Some(queue_names) = &self.queue_names else {
            return self.source.queue_name(queue_id).await;
        };

        queue_names
            .get_or_load(queue_id, || self.source.queue_name(queue_id))
            .await
            .map(|name| name.map(|name| name.as_ref().clone()))
            .map_err(|error| anyhow!("failed to load queue name: {error:#}"))
    }

    async fn queue_details(&self, queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error> {
        self.source.queue_details(queue_id).await
    }

    async fn update_queue(
        &self,
        queue_id: Uuid,
        configuration: &QueueConfigurationUpdate,
    ) -> Result<Option<QueueDetails>, anyhow::Error> {
        let details = self.source.update_queue(queue_id, configuration).await?;

        if let Some(details) = &details
            && let Some(queue_names) = &self.queue_names
            && let Err(error) = queue_names
                .insert(details.id(), Arc::new(details.name().to_owned()))
                .await
        {
            tracing::warn!(
                queue.id = %details.id(),
                error = &error as &dyn std::error::Error,
                "failed to write through in-memory queue-name cache"
            );
        }

        Ok(details)
    }

    async fn enqueue_message(
        &self,
        queue_id: Uuid,
        message: &Message,
        effective_ttl_seconds: u32,
    ) -> Result<(), anyhow::Error> {
        self.source
            .enqueue_message(queue_id, message, effective_ttl_seconds)
            .await
    }

    async fn dequeue_message(
        &self,
        queue_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<DequeueMessageOutcome, anyhow::Error> {
        self.source.dequeue_message(queue_id, receipt_handle).await
    }

    async fn acknowledge_message(
        &self,
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        self.source
            .acknowledge_message(queue_id, message_id, receipt_handle)
            .await
    }

    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, anyhow::Error> {
        self.source.process_expired_messages(batch_size).await
    }
}
