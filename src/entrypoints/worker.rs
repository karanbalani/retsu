use crate::{app::ApplicationContext, configuration::AppConfiguration};

#[tracing::instrument(name = "worker.run", skip_all)]
pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration).await?;

    let result = crate::worker::serve(&context).await;

    context.shutdown().await;

    result
}
