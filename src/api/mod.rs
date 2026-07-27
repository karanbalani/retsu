mod error;
mod extractors;
mod routes;

pub(crate) use error::ApiError;

use std::{io, net::SocketAddr};

use actix_web::{App, HttpServer, middleware::Compress, web};

use crate::{
    app::ApplicationContext,
    http::{
        HttpMetricsMiddleware, HttpTracingMiddleware, RequestIdMiddleware, default_response_headers,
    },
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
            .app_data(extractors::json_config())
            .app_data(extractors::path_config())
            .app_data(extractors::query_config())
            .wrap(Compress::default())
            .wrap(HttpMetricsMiddleware::new(metrics.clone()))
            .wrap(default_response_headers())
            .wrap(HttpTracingMiddleware)
            .wrap(RequestIdMiddleware)
            .configure(routes::configure)
            .default_service(web::to(error::not_found))
    })
    .bind(bind_address)?;

    tracing::info!(%bind_address, "API server listening");

    server.run().await
}

#[cfg(test)]
#[path = "../tests/api.rs"]
mod tests;
