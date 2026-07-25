use crate::{app::ApplicationContext, configuration::AppConfiguration, observability::Metrics};

#[tracing::instrument(name = "worker.run", skip_all)]
pub(crate) async fn run(configuration: AppConfiguration, metrics: Metrics) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration, metrics).await?;

    let shutdown_timeout = configuration.worker.shutdown_timeout();

    let registrations = crate::modules::worker_registraions();

    let result = crate::worker::serve(&context, registrations, shutdown_timeout).await;

    context.shutdown().await;

    result
}
