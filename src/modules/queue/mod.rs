mod api;
mod application;
mod domain;
mod infrastructure;

use actix_web::web;
use sqlx::PgPool;

use application::{
    AcknowledgeMessageCommand, AcknowledgeMessageError, CreateQueueCommand, CreateQueueError,
    CreatedQueue, DequeueMessageCommand, DequeueMessageError, DequeuedMessage,
    EnqueueMessageCommand, EnqueueMessageError, EnqueuedMessage, execute_acknowledge_message,
    execute_create_queue, execute_dequeue_message, execute_enqueue_message,
};
use infrastructure::PostgresQueueRepository;

use crate::observability::{DatabaseMetrics, QueueMetrics};

#[derive(Clone)]
pub(crate) struct QueueModule {
    repository: PostgresQueueRepository,
    queue_metrics: QueueMetrics,
}

impl QueueModule {
    pub(crate) fn new(
        database_pool: PgPool,
        queue_metrics: QueueMetrics,
        database_metrics: DatabaseMetrics,
    ) -> Self {
        Self {
            repository: PostgresQueueRepository::new(database_pool, database_metrics),
            queue_metrics,
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

        self.queue_metrics
            .message_enqueued(message.queue_name(), message.priority());

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
        let queue_name = command.queue_name().to_owned();

        execute_acknowledge_message(&self.repository, command).await?;

        self.queue_metrics.message_acknowledged(&queue_name);

        Ok(())
    }
}

pub(super) fn configure_api(configuration: &mut web::ServiceConfig) {
    api::configure(configuration);
}
