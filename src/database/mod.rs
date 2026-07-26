use std::time::{Duration, Instant};

use sqlx::{
    Error as SqlxError, PgPool, Postgres,
    migrate::{MigrateError, Migrator},
    pool::PoolConnection,
    postgres::PgPoolOptions,
};

use crate::{configuration::DatabaseConfig, observability::DatabaseMetrics};

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

pub(crate) async fn check_health(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

pub(crate) async fn acquire(
    pool: &PgPool,
    metrics: &DatabaseMetrics,
) -> Result<PoolConnection<Postgres>, SqlxError> {
    let started = Instant::now();
    let result = pool.acquire().await;
    let elapsed = started.elapsed();

    metrics.acquisition_finished(elapsed, result.is_ok());

    tracing::Span::current().record("db.pool.acquire.duration", elapsed.as_secs_f64());

    if let Err(error) = &result {
        tracing::Span::current().record("error.type", error_type(error));
        tracing::Span::current().record("otel.status_code", "ERROR");
    }

    result
}

pub(crate) fn error_type(error: &SqlxError) -> &'static str {
    match error {
        SqlxError::Database(_) => "database",
        SqlxError::Io(_) => "io",
        SqlxError::Tls(_) => "tls",
        SqlxError::Protocol(_) => "protocol",
        SqlxError::PoolTimedOut => "pool_timeout",
        SqlxError::PoolClosed => "pool_closed",
        SqlxError::WorkerCrashed => "worker_crashed",
        _ => "other",
    }
}
