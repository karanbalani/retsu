use crate::configuration::AppConfiguration;

#[tracing::instrument(name = "migrate.run", skip_all)]
pub(crate) async fn run(_configuration: AppConfiguration) -> anyhow::Result<()> {
    tracing::info!("starting database migration");
    Ok(())
}
