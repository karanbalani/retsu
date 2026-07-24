use crate::configuration::AppConfiguration;

pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    tracing::info!(environment = %configuration.environment, "starting background worker");
    Ok(())
}
