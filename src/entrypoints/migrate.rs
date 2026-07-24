use crate::configuration::AppConfiguration;

#[tracing::instrument(name = "migrate.run", skip_all)]
pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    let pool = crate::database::connect(&configuration.database).await?;

    let migration_result = crate::database::migrate(&pool).await;

    pool.close().await;

    migration_result?;

    Ok(())
}
