use crate::app::ApplicationContext;

#[tracing::instrument(name = "worker.serve", skip_all)]
pub(crate) async fn serve(_context: &ApplicationContext) -> anyhow::Result<()> {
    tracing::info!("background worker started");
    Ok(())
}
