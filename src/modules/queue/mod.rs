mod api;
mod application;
mod domain;
mod infrastructure;
mod worker;

use std::time::Instant;

use actix_web::web;
use sqlx::PgPool;
use uuid::Uuid;

use super::definition::{ModuleDefinition, WorkerDefinition};

use application::{
    AcknowledgeMessageCommand, AcknowledgeMessageError, CreateQueueCommand, CreateQueueError,
    CreatedQueue, DequeueMessageCommand, DequeueMessageError, DequeuedMessage,
    EnqueueMessageCommand, EnqueueMessageError, EnqueuedMessage, ExpiredMessagesCleanupSummary,
    ProcessExpiredMessagesError, ProcessTimedOutMessagesError, QueueStateRepository,
    TimeoutProcessingSummary, execute_acknowledge_message, execute_create_queue,
    execute_dequeue_message, execute_enqueue_message, execute_process_expired_messages,
    execute_process_timed_out_messages,
};

use crate::{
    cache::{MemoryCache, MemoryCachePolicy},
    configuration::CachePolicyConfig,
    observability::{
        CacheMetrics, DatabaseMetrics, QueueInstrumentation, QueuePriorityStateMetric,
    },
};

use domain::QueueDetails;
use infrastructure::{CachedQueueRepository, PostgresQueueRepository};

type QueueStateCollectorLease = <PostgresQueueRepository as QueueStateRepository>::CollectorLease;
type QueueDetailsMemoryCache = MemoryCache<Uuid, QueueDetails>;
type CachedPostgresQueueRepository =
    CachedQueueRepository<PostgresQueueRepository, QueueDetailsMemoryCache>;

fn queue_details_weight(queue_id: &Uuid, details: &QueueDetails) -> u32 {
    u32::try_from(
        std::mem::size_of_val(queue_id)
            + std::mem::size_of::<QueueDetails>()
            + details.name().len(),
    )
    .unwrap_or(u32::MAX)
}

const WORKERS: &[WorkerDefinition] = &[
    WorkerDefinition::new(
        worker::VISIBILITY_TIMEOUT_PROCESSOR_NAME,
        worker::visibility_timeout_registration,
    ),
    WorkerDefinition::new(
        worker::EXPIRED_MESSAGE_CLEANER_NAME,
        worker::expired_message_cleaner_registration,
    ),
    WorkerDefinition::new(
        worker::STATE_METRICS_COLLECTOR_NAME,
        worker::state_metrics_collector_registration,
    ),
];

pub(super) const DEFINITION: ModuleDefinition = ModuleDefinition::new("queue")
    .with_api(configure_api)
    .with_workers(WORKERS);

#[derive(Clone)]
pub(crate) struct QueueModule {
    postgres_repository: PostgresQueueRepository,
    cached_queue_repository: CachedPostgresQueueRepository,
    instrumentation: QueueInstrumentation,
}

impl QueueModule {
    pub(crate) fn new(
        database_pool: PgPool,
        instrumentation: QueueInstrumentation,
        database_metrics: DatabaseMetrics,
        cache_configuration: &CachePolicyConfig,
        cache_metrics: CacheMetrics,
    ) -> Self {
        let postgres_repository = PostgresQueueRepository::new(database_pool, database_metrics);
        let cache_policy = MemoryCachePolicy::new(
            cache_configuration.max_entries,
            cache_configuration.max_capacity_bytes,
            cache_configuration.time_to_live(),
        );
        let queue_details_cache = MemoryCache::new(
            "queue_details",
            cache_policy,
            queue_details_weight,
            cache_metrics,
        );
        let cached_queue_repository =
            CachedQueueRepository::new(postgres_repository.clone(), queue_details_cache);

        Self {
            postgres_repository,
            cached_queue_repository,
            instrumentation,
        }
    }

    async fn create_queue(
        &self,
        command: CreateQueueCommand,
    ) -> Result<CreatedQueue, CreateQueueError> {
        execute_create_queue(&self.cached_queue_repository, command).await
    }

    async fn enqueue_message(
        &self,
        command: EnqueueMessageCommand,
    ) -> Result<EnqueuedMessage, EnqueueMessageError> {
        let queue_id = command.queue_id();
        let message = match execute_enqueue_message(
            &self.cached_queue_repository,
            &self.postgres_repository,
            command,
        )
        .await
        {
            Ok(message) => message,
            Err(error @ EnqueueMessageError::QueueNotFound) => {
                self.cached_queue_repository
                    .invalidate_queue_details(queue_id)
                    .await;

                return Err(error);
            }
            Err(error) => return Err(error),
        };

        self.instrumentation
            .commands()
            .message_enqueued(message.queue_name(), message.priority());

        Ok(message)
    }

    async fn dequeue_message(
        &self,
        command: DequeueMessageCommand,
    ) -> Result<Option<DequeuedMessage>, DequeueMessageError> {
        execute_dequeue_message(&self.postgres_repository, command).await
    }

    async fn acknowledge_message(
        &self,
        command: AcknowledgeMessageCommand,
    ) -> Result<(), AcknowledgeMessageError> {
        let queue_id = command.queue_id();
        let message = match execute_acknowledge_message(
            &self.cached_queue_repository,
            &self.postgres_repository,
            command,
        )
        .await
        {
            Ok(message) => message,
            Err(error @ AcknowledgeMessageError::QueueNotFound) => {
                self.cached_queue_repository
                    .invalidate_queue_details(queue_id)
                    .await;

                return Err(error);
            }
            Err(error) => return Err(error),
        };

        self.instrumentation
            .commands()
            .message_acknowledged(message.queue_name());

        Ok(())
    }

    async fn process_timed_out_messages(
        &self,
        batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, ProcessTimedOutMessagesError> {
        let summary =
            execute_process_timed_out_messages(&self.postgres_repository, batch_size).await?;
        let metrics = self.instrumentation.visibility_timeout();

        for queue in summary.per_queue() {
            metrics.messages_requeued(queue.queue_name(), queue.requeued());
            metrics.messages_dead_lettered(queue.queue_name(), queue.dead_lettered());
        }

        Ok(summary)
    }

    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, ProcessExpiredMessagesError> {
        let summary =
            execute_process_expired_messages(&self.postgres_repository, batch_size).await?;
        let metrics = self.instrumentation.expired_message_cleaner();

        for queue in summary.per_queue() {
            metrics.messages_expired(
                queue.queue_name(),
                "never_delivered",
                queue.never_delivered(),
            );

            metrics.messages_expired(
                queue.queue_name(),
                "previously_delivered",
                queue.previously_delivered(),
            );
        }

        Ok(summary)
    }

    async fn try_acquire_state_collector_lease(
        &self,
    ) -> Result<Option<QueueStateCollectorLease>, anyhow::Error> {
        self.postgres_repository.try_acquire_collector_lease().await
    }

    async fn refresh_state_metrics(
        &self,
        lease: &mut QueueStateCollectorLease,
    ) -> Result<(), anyhow::Error> {
        let metrics = self.instrumentation.state();
        let started = Instant::now();
        let result = self.postgres_repository.queue_state(lease).await;

        match result {
            Ok(snapshot) => {
                let measurements = snapshot
                    .into_iter()
                    .map(|state| {
                        QueuePriorityStateMetric::new(
                            state.queue_name().to_owned(),
                            state.priority().as_str(),
                            state.ready(),
                            state.in_flight(),
                            state.oldest_ready_age_seconds(),
                            state.oldest_in_flight_age_seconds(),
                        )
                    })
                    .collect();

                metrics.replace(measurements);
                metrics.collection_finished(started.elapsed(), true);

                Ok(())
            }

            Err(error) => {
                metrics.collection_finished(started.elapsed(), false);

                Err(error)
            }
        }
    }
}

pub(super) fn configure_api(configuration: &mut web::ServiceConfig) {
    api::configure(configuration);
}
