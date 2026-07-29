use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time;
use uuid::Uuid;

use crate::{
    cache::{CacheError, RedisProtocolCommands},
    observability::CacheMetrics,
};

use super::super::{
    application::{
        AcknowledgeMessageOutcome, CreateQueueOutcome, DeadLetterMessagesPurgeSummary,
        DequeueMessageOutcome, ExpiredMessagesCleanupSummary, QueueRepository,
    },
    domain::{Message, Queue, QueueConfigurationUpdate, QueueDetails},
};

const CACHE_NAME: &str = "queue_details";
const KEY_PREFIX: &str = "retsu:queue_details";
const DETAILS_TTL: Duration = Duration::from_secs(300);
const MISSING_TTL: Duration = Duration::from_secs(5);
const LOAD_LOCK_TTL: Duration = Duration::from_secs(2);
const LOAD_RETRY_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum CachedQueueDetails {
    Found(QueueDetails),
    Missing,
}

#[derive(Clone)]
pub(in crate::modules::queue) struct L2QueueRepository<R, C> {
    source: R,
    cache: Option<C>,
    metrics: CacheMetrics,
}

impl<R, C> L2QueueRepository<R, C> {
    pub(in crate::modules::queue) fn new(
        source: R,
        cache: Option<C>,
        metrics: CacheMetrics,
    ) -> Self {
        Self {
            source,
            cache,
            metrics,
        }
    }
}

impl<R, C> L2QueueRepository<R, C>
where
    C: RedisProtocolCommands,
{
    async fn read(
        &self,
        cache: &C,
        queue_id: Uuid,
    ) -> Result<Option<CachedQueueDetails>, CacheError> {
        let value = cache.get(&details_key(queue_id)).await?;

        match value {
            Some(value) => {
                self.metrics.request(CACHE_NAME, "hit");
                serde_json::from_slice(&value)
                    .map(Some)
                    .map_err(CacheError::new)
            }
            None => {
                self.metrics.request(CACHE_NAME, "miss");
                Ok(None)
            }
        }
    }

    async fn write(
        &self,
        cache: &C,
        queue_id: Uuid,
        details: &CachedQueueDetails,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let value = serde_json::to_vec(details).map_err(CacheError::new)?;

        cache.set(&details_key(queue_id), &value, ttl).await
    }

    async fn insert(&self, cache: &C, details: &QueueDetails) -> Result<(), CacheError> {
        self.write(
            cache,
            details.id(),
            &CachedQueueDetails::Found(details.clone()),
            DETAILS_TTL,
        )
        .await
    }

    fn log_degraded_read(&self, queue_id: Uuid, error: &CacheError) {
        tracing::warn!(
            queue.id = %queue_id,
            error = error as &dyn std::error::Error,
            "distributed queue-details cache unavailable; loading from PostgreSQL"
        );
    }
}

impl<R, C> L2QueueRepository<R, C>
where
    R: QueueRepository,
    C: RedisProtocolCommands,
{
    async fn load_and_cache(
        &self,
        cache: &C,
        queue_id: Uuid,
    ) -> Result<Option<QueueDetails>, anyhow::Error> {
        let started = std::time::Instant::now();
        let result = self.source.queue_details(queue_id).await;

        match &result {
            Ok(Some(details)) => {
                self.metrics
                    .load_finished(CACHE_NAME, started.elapsed(), "success");

                if let Err(error) = self.insert(cache, details).await {
                    tracing::warn!(
                        queue.id = %details.id(),
                        error = &error as &dyn std::error::Error,
                        "failed to populate distributed queue-details cache"
                    );
                }
            }
            Ok(None) => {
                self.metrics
                    .load_finished(CACHE_NAME, started.elapsed(), "not_found");

                if let Err(error) = self
                    .write(cache, queue_id, &CachedQueueDetails::Missing, MISSING_TTL)
                    .await
                {
                    tracing::warn!(
                        queue.id = %queue_id,
                        error = &error as &dyn std::error::Error,
                        "failed to populate distributed missing-queue cache entry"
                    );
                }
            }
            Err(_) => {
                self.metrics
                    .load_finished(CACHE_NAME, started.elapsed(), "error");
            }
        }

        result
    }
}

impl<R, C> QueueRepository for L2QueueRepository<R, C>
where
    R: QueueRepository,
    C: RedisProtocolCommands,
{
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        let outcome = self.source.create_queue(queue).await?;

        if matches!(outcome, CreateQueueOutcome::Created)
            && let Some(cache) = &self.cache
            && let Err(error) = self.insert(cache, &queue.details()).await
        {
            tracing::warn!(
                queue.id = %queue.id(),
                error = &error as &dyn std::error::Error,
                "failed to write through distributed queue-details cache"
            );
        }

        Ok(outcome)
    }

    async fn queue_name(&self, queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        if self.cache.is_none() {
            return self.source.queue_name(queue_id).await;
        }

        self.queue_details(queue_id)
            .await
            .map(|details| details.map(|details| details.name().to_owned()))
    }

    async fn queue_details(&self, queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error> {
        let Some(cache) = &self.cache else {
            return self.source.queue_details(queue_id).await;
        };

        match self.read(cache, queue_id).await {
            Ok(Some(details)) => return Ok(details.into_details()),
            Ok(None) => {}
            Err(error) => {
                self.log_degraded_read(queue_id, &error);
                return self.source.queue_details(queue_id).await;
            }
        }

        let lock_key = load_lock_key(queue_id);

        loop {
            match cache.set_if_absent(&lock_key, b"1", LOAD_LOCK_TTL).await {
                Ok(true) => match self.read(cache, queue_id).await {
                    Ok(Some(details)) => return Ok(details.into_details()),
                    Ok(None) => return self.load_and_cache(cache, queue_id).await,
                    Err(error) => {
                        self.log_degraded_read(queue_id, &error);
                        return self.source.queue_details(queue_id).await;
                    }
                },
                Ok(false) => {
                    time::sleep(LOAD_RETRY_INTERVAL).await;

                    match self.read(cache, queue_id).await {
                        Ok(Some(details)) => return Ok(details.into_details()),
                        Ok(None) => {}
                        Err(error) => {
                            self.log_degraded_read(queue_id, &error);
                            return self.source.queue_details(queue_id).await;
                        }
                    }
                }
                Err(error) => {
                    self.log_degraded_read(queue_id, &error);
                    return self.source.queue_details(queue_id).await;
                }
            }
        }
    }

    async fn update_queue(
        &self,
        queue_id: Uuid,
        configuration: &QueueConfigurationUpdate,
    ) -> Result<Option<QueueDetails>, anyhow::Error> {
        let details = self.source.update_queue(queue_id, configuration).await?;

        if let Some(details) = &details
            && let Some(cache) = &self.cache
            && let Err(error) = self.insert(cache, details).await
        {
            tracing::warn!(
                queue.id = %details.id(),
                error = &error as &dyn std::error::Error,
                "failed to write through distributed queue-details cache"
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

    async fn purge_dead_letter_messages(
        &self,
        retention_seconds: i64,
        batch_size: u32,
    ) -> Result<DeadLetterMessagesPurgeSummary, anyhow::Error> {
        self.source
            .purge_dead_letter_messages(retention_seconds, batch_size)
            .await
    }
}

impl CachedQueueDetails {
    fn into_details(self) -> Option<QueueDetails> {
        match self {
            Self::Found(details) => Some(details),
            Self::Missing => None,
        }
    }
}

fn details_key(queue_id: Uuid) -> String {
    format!("{KEY_PREFIX}:{queue_id}")
}

fn load_lock_key(queue_id: Uuid) -> String {
    format!("{}:load-lock", details_key(queue_id))
}
