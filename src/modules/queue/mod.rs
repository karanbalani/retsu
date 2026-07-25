mod api;
mod application;
mod domain;
mod infrastructure;

use actix_web::web;
use sqlx::PgPool;

use application::{CreateQueueCommand, CreateQueueError, CreatedQueue, execute_create_queue};
use infrastructure::PostgresQueueRepository;

#[derive(Clone)]
pub(crate) struct QueueModule {
    repository: PostgresQueueRepository,
}

impl QueueModule {
    pub(crate) fn new(database_pool: PgPool) -> Self {
        Self {
            repository: PostgresQueueRepository::new(database_pool),
        }
    }

    async fn create_queue(
        &self,
        command: CreateQueueCommand,
    ) -> Result<CreatedQueue, CreateQueueError> {
        execute_create_queue(&self.repository, command).await
    }
}

pub(super) fn configure_api(configuration: &mut web::ServiceConfig) {
    api::configure(configuration);
}
