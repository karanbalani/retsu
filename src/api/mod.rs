use std::{io, net::SocketAddr};

use actix_web::{App, HttpServer};

#[tracing::instrument(
    name = "api.serve",
    skip_all,
    fields(bind_address = %bind_address)
)]
pub(crate) async fn serve(bind_address: SocketAddr) -> io::Result<()> {
    let server = HttpServer::new(App::new).bind(bind_address)?;

    tracing::info!(%bind_address, "API server listening");

    server.run().await
}
