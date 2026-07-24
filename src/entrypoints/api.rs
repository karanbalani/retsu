use crate::{app::ApplicationContext, configuration::AppConfiguration};

#[tracing::instrument(
    name = "api.run",
    skip_all,
    fields(bind_address = %configuration.http.socket_address())
)]
pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration).await?;
    let bind_address = configuration.http.socket_address();

    let result = crate::api::serve(&context, bind_address).await;

    context.shutdown().await;
    result?;

    Ok(())
}
