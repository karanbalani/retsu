use sqlx::PgPool;

use crate::{
    configuration::AppConfiguration, database, modules::QueueModule, observability::Metrics,
};

#[derive(Clone)]
pub(crate) struct ApplicationContext {
    database_pool: PgPool,
    metrics: Metrics,
    queue_module: QueueModule,
}

impl ApplicationContext {
    #[tracing::instrument(name = "application.initialize", skip_all, err)]
    pub(crate) async fn initialize(
        configuration: &AppConfiguration,
        metrics: Metrics,
    ) -> Result<Self, sqlx::Error> {
        let database_pool = database::connect(&configuration.database).await?;

        metrics.database().register_pool(
            database_pool.clone(),
            configuration.database.max_connections,
        );

        let queue_module = QueueModule::new(
            database_pool.clone(),
            metrics.queue().clone(),
            metrics.database().clone(),
            &configuration.cache.queue_details,
            metrics.cache().clone(),
        );

        tracing::info!("application dependencies initialized");

        Ok(Self {
            database_pool,
            metrics,
            queue_module,
        })
    }

    #[tracing::instrument(name = "application.shutdown", skip_all)]
    pub(crate) async fn shutdown(self) {
        self.database_pool.close().await;

        tracing::info!("application dependencies shut down")
    }

    #[tracing::instrument(name = "application.readiness", skip_all)]
    pub(crate) async fn check_readiness(&self) -> Result<(), sqlx::Error> {
        database::check_health(&self.database_pool).await
    }

    pub(crate) fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub(crate) fn queue_module(&self) -> &QueueModule {
        &self.queue_module
    }
}
