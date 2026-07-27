use crate::{
    app::ApplicationContext, configuration::AppConfiguration, observability::Metrics,
    worker::WorkerRegistration,
};

#[tracing::instrument(
    name = "worker.run",
    skip_all,
    fields(worker.name = %registration.name)
)]
pub(crate) async fn run(
    configuration: AppConfiguration,
    metrics: Metrics,
    registration: WorkerRegistration,
) -> anyhow::Result<()> {
    let context = ApplicationContext::initialize(&configuration, metrics).await?;

    let shutdown_timeout = configuration.worker.shutdown_timeout();
    let management_address = configuration.worker.management.socket_address();

    let registrations = vec![
        registration,
        crate::worker::management_registration(management_address),
    ];

    let result = crate::worker::serve(&context, registrations, shutdown_timeout).await;

    context.shutdown().await;

    result
}
