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
mod tests {
    use actix_web::{
        App, HttpResponse,
        http::{Method, StatusCode, header::ALLOW},
        test, web,
    };
    use serde_json::Value;

    use super::error;

    #[actix_web::test]
    async fn unknown_routes_use_the_api_problem_contract() {
        let app = test::init_service(App::new().default_service(web::to(error::not_found))).await;

        let response = test::call_service(
            &app,
            test::TestRequest::get().uri("/not-a-route").to_request(),
        )
        .await;
        let status = response.status();
        let body: Value = test::read_body_json(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "route_not_found");
    }

    #[actix_web::test]
    async fn known_routes_preserve_actix_method_not_allowed_responses() {
        let app = test::init_service(
            App::new()
                .service(web::resource("/known").route(web::get().to(HttpResponse::NoContent)))
                .default_service(web::to(error::not_found)),
        )
        .await;
        let request = test::TestRequest::default()
            .method(Method::POST)
            .uri("/known")
            .to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response.headers().get(ALLOW),
            Some(&"GET".parse().expect("valid Allow header"))
        );
    }
}
