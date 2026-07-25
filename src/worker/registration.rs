use std::pin::Pin;

use tokio_util::sync::CancellationToken;

use crate::app::ApplicationContext;

pub(crate) type WorkerFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

pub(crate) type WorkerRunner =
    Box<dyn FnOnce(ApplicationContext, CancellationToken) -> WorkerFuture + Send + 'static>;

pub(crate) struct WorkerRegistration {
    pub(crate) name: &'static str,
    pub(crate) run: WorkerRunner,
}
