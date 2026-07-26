use std::net::SocketAddr;

use actix_web::{App, HttpServer, middleware::Compress, web};
use anyhow::Context as _;
use tokio_util::sync::CancellationToken;

use crate::{
    app::ApplicationContext,
    http::{HttpMetricsMiddleware, RequestIdMiddleware, default_response_headers},
    management,
    worker::WorkerRegistration,
};

pub(crate) fn registration(bind_address: SocketAddr) -> WorkerRegistration {
    WorkerRegistration {
        name: "management_http",
        run: Box::new(move |context, cancellation| {
            Box::pin(serve(context, bind_address, cancellation))
        }),
    }
}

async fn serve(
    context: ApplicationContext,
    bind_address: SocketAddr,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    let metrics = context.metrics().clone();
    let context = web::Data::new(context);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(context.clone())
            .wrap(Compress::default())
            .wrap(HttpMetricsMiddleware::new(metrics.clone()))
            .wrap(default_response_headers())
            .wrap(RequestIdMiddleware)
            .configure(management::configure)
    })
    .keep_alive(None)
    .workers(1)
    .disable_signals()
    .bind(bind_address)
    .with_context(|| format!("failed to bind worker management listener to {bind_address}"))?
    .run();

    let handle = server.handle();
    tokio::pin!(server);

    tracing::info!(%bind_address, "worker management server listening");

    tokio::select! {
        result = &mut server => {
            result.context("worker management server failed")?;
        }

        () = cancellation.cancelled() => {
            tracing::info!("stopping worker management server");

            let (_, result) = tokio::join!(handle.stop(true), &mut server);

            result.context("worker management server failed during shutdown")?;
        }
    }

    Ok(())
}
