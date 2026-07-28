mod api;
mod application;
mod domain;
mod infrastructure;
mod worker;

use std::time::Instant;

use actix_web::web;
use sqlx::PgPool;

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

use crate::observability::{DatabaseMetrics, QueueInstrumentation, QueuePriorityStateMetric};

use infrastructure::PostgresQueueRepository;

type QueueStateCollectorLease = <PostgresQueueRepository as QueueStateRepository>::CollectorLease;

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
    repository: PostgresQueueRepository,
    instrumentation: QueueInstrumentation,
}

impl QueueModule {
    pub(crate) fn new(
        database_pool: PgPool,
        instrumentation: QueueInstrumentation,
        database_metrics: DatabaseMetrics,
    ) -> Self {
        Self {
            repository: PostgresQueueRepository::new(database_pool, database_metrics),
            instrumentation,
        }
    }

    async fn create_queue(
        &self,
        command: CreateQueueCommand,
    ) -> Result<CreatedQueue, CreateQueueError> {
        execute_create_queue(&self.repository, command).await
    }

    async fn enqueue_message(
        &self,
        command: EnqueueMessageCommand,
    ) -> Result<EnqueuedMessage, EnqueueMessageError> {
        let message = execute_enqueue_message(&self.repository, command).await?;

        self.instrumentation
            .commands()
            .message_enqueued(message.queue_id(), message.priority());

        Ok(message)
    }

    async fn dequeue_message(
        &self,
        command: DequeueMessageCommand,
    ) -> Result<Option<DequeuedMessage>, DequeueMessageError> {
        execute_dequeue_message(&self.repository, command).await
    }

    async fn acknowledge_message(
        &self,
        command: AcknowledgeMessageCommand,
    ) -> Result<(), AcknowledgeMessageError> {
        let queue_id = command.queue_id();

        execute_acknowledge_message(&self.repository, command).await?;

        self.instrumentation
            .commands()
            .message_acknowledged(queue_id);

        Ok(())
    }

    async fn process_timed_out_messages(
        &self,
        batch_size: u32,
    ) -> Result<TimeoutProcessingSummary, ProcessTimedOutMessagesError> {
        let summary = execute_process_timed_out_messages(&self.repository, batch_size).await?;
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
        let summary = execute_process_expired_messages(&self.repository, batch_size).await?;
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
        self.repository.try_acquire_collector_lease().await
    }

    async fn refresh_state_metrics(
        &self,
        lease: &mut QueueStateCollectorLease,
    ) -> Result<(), anyhow::Error> {
        let metrics = self.instrumentation.state();
        let started = Instant::now();
        let result = self.repository.queue_state(lease).await;

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
