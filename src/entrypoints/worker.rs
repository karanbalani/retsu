use crate::{app::ApplicationContext, configuration::AppConfiguration, observability::Metrics};

#[tracing::instrument(name = "worker.run", skip_all,fields(
        worker.module = %worker_module,
        worker.name = %worker_name,
    ))]
pub(crate) async fn run(
    configuration: AppConfiguration,
    metrics: Metrics,
    worker_module: String,
    worker_name: String,
) -> anyhow::Result<()> {
    let domain_registration = crate::modules::worker_registration(&worker_module, &worker_name)?;

    let context = ApplicationContext::initialize(&configuration, metrics).await?;

    let shutdown_timeout = configuration.worker.shutdown_timeout();

    let management_address = configuration.worker.management.socket_address();

    let registrations = vec![
        domain_registration,
        crate::worker::management_registration(management_address),
    ];

    let result = crate::worker::serve(&context, registrations, shutdown_timeout).await;

    context.shutdown().await;

    result
}
