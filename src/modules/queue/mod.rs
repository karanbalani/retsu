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
    DequeueMessageCommand, DequeueMessageError, DequeuedMessage, EnqueueMessageCommand,
    EnqueueMessageError, EnqueuedMessage, ExpiredMessagesCleanupSummary,
    ProcessExpiredMessagesError, ProcessTimedOutMessagesError, TimeoutProcessingSummary,
    UpdateQueueCommand, UpdateQueueError, execute_acknowledge_message, execute_create_queue,
    execute_dequeue_message, execute_enqueue_message, execute_process_expired_messages,
    execute_process_timed_out_messages, execute_update_queue,
};

use crate::{
    cache::{MemoryCache, MemoryCachePolicy, RedisProtocolCache},
    configuration::{DistributedCacheConfig, InMemoryCacheConfig},
    observability::{
        CacheMetrics, DatabaseMetrics, QueueInstrumentation, QueuePriorityStateMetric,
    },
};

use infrastructure::{
    L1QueueRepository, L2QueueRepository, PostgresQueueRepository, PostgresQueueStateCollector,
    QueueStateCollectorLease,
};

use domain::QueueDetails;

type L2PostgresQueueRepository = L2QueueRepository<PostgresQueueRepository, RedisProtocolCache>;
type QueueRepositoryChain = L1QueueRepository<L2PostgresQueueRepository>;

fn queue_name_weight(queue_id: &Uuid, queue_name: &String) -> u32 {
    u32::try_from(
        std::mem::size_of_val(queue_id) + std::mem::size_of::<String>() + queue_name.capacity(),
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
    queue_repository: QueueRepositoryChain,
    state_collector: PostgresQueueStateCollector,
    instrumentation: QueueInstrumentation,
}

impl QueueModule {
    pub(crate) fn new(
        database_pool: PgPool,
        instrumentation: QueueInstrumentation,
        database_metrics: DatabaseMetrics,
        in_memory_cache_configuration: &InMemoryCacheConfig,
        distributed_cache_configuration: &DistributedCacheConfig,
        cache_metrics: CacheMetrics,
    ) -> anyhow::Result<Self> {
        let state_collector =
            PostgresQueueStateCollector::new(database_pool.clone(), database_metrics.clone());
        let postgres_repository = PostgresQueueRepository::new(database_pool, database_metrics);

        let queue_name_cache = in_memory_cache_configuration.enabled.then(|| {
            let configuration = &in_memory_cache_configuration.regions.queue_names;
            let policy =
                MemoryCachePolicy::new(configuration.max_entries, configuration.max_capacity_bytes);

            MemoryCache::new(
                "queue_names",
                policy,
                queue_name_weight,
                cache_metrics.clone(),
            )
        });

        let distributed_cache = if distributed_cache_configuration.enabled {
            Some(RedisProtocolCache::new(distributed_cache_configuration)?)
        } else {
            None
        };

        let l2_repository =
            L2QueueRepository::new(postgres_repository, distributed_cache, cache_metrics);
        let queue_repository = L1QueueRepository::new(l2_repository, queue_name_cache);

        Ok(Self {
            queue_repository,
            state_collector,
            instrumentation,
        })
    }

    async fn create_queue(
        &self,
        command: CreateQueueCommand,
    ) -> Result<QueueDetails, CreateQueueError> {
        execute_create_queue(&self.queue_repository, command).await
    }

    async fn update_queue(
        &self,
        command: UpdateQueueCommand,
    ) -> Result<QueueDetails, UpdateQueueError> {
        execute_update_queue(&self.queue_repository, command).await
    }

    async fn enqueue_message(
        &self,
        command: EnqueueMessageCommand,
    ) -> Result<EnqueuedMessage, EnqueueMessageError> {
        let message = execute_enqueue_message(&self.queue_repository, command).await?;

        self.instrumentation
            .commands()
            .message_enqueued(message.queue_name(), message.priority());

        Ok(message)
    }

    async fn dequeue_message(
        &self,
        command: DequeueMessageCommand,
    ) -> Result<Option<DequeuedMessage>, DequeueMessageError> {
        execute_dequeue_message(&self.queue_repository, command).await
    }

    async fn acknowledge_message(
        &self,
        command: AcknowledgeMessageCommand,
    ) -> Result<(), AcknowledgeMessageError> {
        if let Some(message) = execute_acknowledge_message(&self.queue_repository, command).await? {
            self.instrumentation
                .commands()
                .message_acknowledged(message.queue_name());
        }

        Ok(())
    }

    async fn process_timed_out_messages(
        &self,
        batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, ProcessTimedOutMessagesError> {
        let summary =
            execute_process_timed_out_messages(&self.queue_repository, batch_size).await?;
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
        let summary = execute_process_expired_messages(&self.queue_repository, batch_size).await?;
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
        self.state_collector.try_acquire_lease().await
    }

    async fn refresh_state_metrics(
        &self,
        lease: &mut QueueStateCollectorLease,
    ) -> Result<(), anyhow::Error> {
        let metrics = self.instrumentation.state();
        let started = Instant::now();
        let result = self.state_collector.collect(lease).await;

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
