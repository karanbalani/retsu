mod api;
mod application;
mod domain;
mod infrastructure;

use actix_web::web;
use sqlx::PgPool;

use application::{
    CreateQueueCommand, CreateQueueError, CreatedQueue, EnqueueMessageCommand, EnqueueMessageError,
    EnqueuedMessage, execute_create_queue, execute_enqueue_message,
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
}

pub(super) fn configure_api(configuration: &mut web::ServiceConfig) {
    api::configure(configuration);
}
