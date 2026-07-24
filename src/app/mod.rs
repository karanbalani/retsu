use sqlx::PgPool;

use crate::{configuration::AppConfiguration, database};

#[derive(Clone)]
pub(crate) struct ApplicationContext {
    database_pool: PgPool,
}

impl ApplicationContext {
    #[tracing::instrument(name = "application.initialize", skip_all, err)]
    pub(crate) async fn initialize(configuration: &AppConfiguration) -> Result<Self, sqlx::Error> {
        let database_pool = database::connect(&configuration.database).await?;

        tracing::info!("application dependencies initialized");

        Ok(Self { database_pool })
    }

    #[tracing::instrument(name = "application.shutdown", skip_all)]
    pub(crate) async fn shutdown(self) {
        self.database_pool.close().await;

        tracing::info!("application dependencies shut down")
    }
}
