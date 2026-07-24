use crate::configuration::AppConfiguration;

#[tracing::instrument(name = "worker.run", skip_all)]
pub(crate) async fn run(_configuration: AppConfiguration) -> anyhow::Result<()> {
    tracing::info!("starting background worker");
    Ok(())
}
