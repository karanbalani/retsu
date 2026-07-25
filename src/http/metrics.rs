use std::{
    future::{Ready, ready},
    pin::Pin,
    time::Instant,
};

use actix_web::{
    Error,
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};

use super::request::{normalized_method, normalized_scheme, protocol_version};

use crate::observability::Metrics;

pub(crate) struct HttpMetricsMiddleware {
    metrics: Metrics,
}

impl HttpMetricsMiddleware {
    pub(crate) fn new(metrics: Metrics) -> Self {
        Self { metrics }
    }
}

impl<S, B> Transform<S, ServiceRequest> for HttpMetricsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = HttpMetricsService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HttpMetricsService {
            service,
            metrics: self.metrics.clone(),
        }))
    }
}

pub(crate) struct HttpMetricsService<S> {
    service: S,
    metrics: Metrics,
}

impl<S, B> Service<ServiceRequest> for HttpMetricsService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let should_record = request.path() != "/metrics";
        let method = normalized_method(request.method().as_str()).to_owned();
        let scheme = normalized_scheme(request.connection_info().scheme()).to_owned();
        let protocol_version = protocol_version(request.version());
        let started_at = Instant::now();

        let active_request = should_record.then(|| {
            self.metrics
                .http()
                .request_started(method.clone(), scheme.clone())
        });

        let metrics = self.metrics.clone();
        let future = self.service.call(request);

        Box::pin(async move {
            let result = future.await;

            if should_record {
                let (status_code, route) = match &result {
                    Ok(response) => (
                        response.status().as_u16(),
                        response.request().match_pattern(),
                    ),
                    Err(error) => (error.as_response_error().status_code().as_u16(), None),
                };

                metrics.http().request_finished(
                    method,
                    scheme,
                    protocol_version,
                    route,
                    status_code,
                    started_at.elapsed(),
                );
            }

            drop(active_request);

            result
        })
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{App, HttpResponse, test, web};

    use crate::observability::test_metrics;

    use super::HttpMetricsMiddleware;

    #[actix_web::test]
    async fn records_matched_routes_and_excludes_the_metrics_path() {
        let (provider, metrics) = test_metrics();
        let app = test::init_service(
            App::new()
                .wrap(HttpMetricsMiddleware::new(metrics.clone()))
                .route(
                    "/items/{identifier}",
                    web::get().to(HttpResponse::NoContent),
                )
                .route("/metrics", web::get().to(HttpResponse::Ok)),
        )
        .await;

        let item_response =
            test::call_service(&app, test::TestRequest::get().uri("/items/42").to_request()).await;
        assert!(item_response.status().is_success());

        let metrics_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/metrics?probe=true")
                .to_request(),
        )
        .await;
        assert!(metrics_response.status().is_success());

        let output = String::from_utf8(metrics.encode_prometheus().expect("metrics should encode"))
            .expect("metrics should be UTF-8");
        let count_lines = output
            .lines()
            .filter(|line| line.starts_with("http_server_request_duration_seconds_count"))
            .collect::<Vec<_>>();

        assert_eq!(count_lines.len(), 1);
        assert!(count_lines[0].contains("http_request_method=\"GET\""));
        assert!(count_lines[0].contains("http_route=\"/items/{identifier}\""));
        assert!(count_lines[0].contains("http_response_status_code=\"204\""));
        assert!(count_lines[0].contains("network_protocol_version=\"1.1\""));
        assert!(count_lines[0].contains("url_scheme=\"http\""));
        assert!(count_lines[0].ends_with(" 1"));

        provider.shutdown().expect("provider should shut down");
    }
}
