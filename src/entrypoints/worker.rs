use crate::{app::ApplicationContext, configuration::AppConfiguration, observability::Metrics};

#[tracing::instrument(name = "worker.run", skip_all)]
pub(crate) async fn run(configuration: AppConfiguration, metrics: Metrics) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration, metrics).await?;

    let shutdown_timeout = configuration.worker.shutdown_timeout();

    let management_address = configuration.worker.management.socket_address();

    let mut registrations = crate::modules::worker_registrations();

    if registrations.is_empty() {
        tracing::warn!("no domain background workers registered");
    }

    registrations.push(crate::worker::management_registration(management_address));

    let result = crate::worker::serve(&context, registrations, shutdown_timeout).await;

    context.shutdown().await;

    result
}
