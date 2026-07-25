mod routes;

use std::{io, net::SocketAddr};

use actix_web::{App, HttpServer, web};

use crate::{
    app::ApplicationContext,
    http::{HttpMetricsMiddleware, HttpTracingMiddleware},
};

#[tracing::instrument(
    name = "api.serve",
    skip_all,
    fields(bind_address = %bind_address)
)]
pub(crate) async fn serve(
    context: &ApplicationContext,
    bind_address: SocketAddr,
) -> io::Result<()> {
    let metrics = context.metrics().clone();
    let context = web::Data::new(context.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(context.clone())
            .wrap(HttpMetricsMiddleware::new(metrics.clone()))
            .wrap(HttpTracingMiddleware)
            .configure(routes::configure)
    })
    .bind(bind_address)?;

    tracing::info!(%bind_address, "API server listening");

    server.run().await
}
