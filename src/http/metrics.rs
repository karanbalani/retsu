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
#[path = "../tests/http_metrics.rs"]
mod tests;
