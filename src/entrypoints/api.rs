use crate::configuration::AppConfiguration;

pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    tracing::info!(environment = %configuration.environment, socket_address = %configuration.http.socket_address(), "starting api server");
    Ok(())
}
