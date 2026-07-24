use std::time::Duration;

use sqlx::{
    PgPool,
    migrate::{MigrateError, Migrator},
    postgres::PgPoolOptions,
};

use crate::configuration::DatabaseConfig;

static MIGRATOR: Migrator = sqlx::migrate!();

#[tracing::instrument(
    name = "database.connect",
    skip_all,
    fields(max_connections = configuration.max_connections)
)]
pub(crate) async fn connect(configuration: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(configuration.max_connections)
        .acquire_timeout(Duration::from_secs(configuration.acquire_timeout_seconds))
        .connect(&configuration.url)
        .await?;

    tracing::info!("database connection pool established");

    Ok(pool)
}

#[tracing::instrument(name = "database.migrate", skip_all)]
pub(crate) async fn migrate(pool: &PgPool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await?;
    tracing::info!("database migrations applied");
    Ok(())
}
