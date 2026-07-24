use crate::{app::ApplicationContext, configuration::AppConfiguration, observability::Metrics};

#[tracing::instrument(name = "worker.run", skip_all)]
pub(crate) async fn run(configuration: AppConfiguration, metrics: Metrics) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration, metrics).await?;

    let result = crate::worker::serve(&context).await;

    context.shutdown().await;

    result
}
