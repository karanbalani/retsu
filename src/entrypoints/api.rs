use crate::configuration::AppConfiguration;

#[tracing::instrument(
    name = "api.run",
    skip_all,
    fields(bind_address = %configuration.http.socket_address())
)]
pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    tracing::info!("starting api server");
    Ok(())
}
