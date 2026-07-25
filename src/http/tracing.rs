use std::{
    future::{Ready, ready},
    pin::Pin,
};

use actix_web::{
    Error, HttpMessage as _,
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::HeaderMap,
};
use opentelemetry::{
    global,
    propagation::Extractor,
    trace::{Status, TraceContextExt as _},
};
use tokio::time::Instant;
use tracing::{Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::http::{
    RequestId,
    request::{normalized_method, normalized_scheme, protocol_version},
};

pub(crate) struct HttpTracingMiddleware;

impl<S, B> Transform<S, ServiceRequest> for HttpTracingMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = HttpTracingService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HttpTracingService { service }))
    }
}

pub(crate) struct HttpTracingService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for HttpTracingService<S>
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
        if request.path() == "/metrics" {
            return Box::pin(self.service.call(request));
        }

        let method = normalized_method(request.method().as_str()).to_owned();
        let scheme = normalized_scheme(request.connection_info().scheme()).to_owned();
        let protocol_version = protocol_version(request.version());
        let started_at = Instant::now();
        let request_id = request.extensions().get::<RequestId>().cloned();

        let parent_context = global::get_text_map_propagator(|propogator| {
            propogator.extract(&HeaderExtractor(request.headers()))
        });

        let span = tracing::info_span!(
            "http.request",
            "otel.kind" = "server",
            "otel.name" = %method,
            "http.request.method" = %method,
            "url.scheme" = %scheme,
            "network.protocol.version" = tracing::field::Empty,
            "http.route" = tracing::field::Empty,
            "http.response.status_code" = tracing::field::Empty,
            "error.type" = tracing::field::Empty,
            trace_id = tracing::field::Empty,
            span_id = tracing::field::Empty,
            "request.id" = tracing::field::Empty
        );

        let _ = span.set_parent(parent_context);

        if let Some(request_id) = request_id {
            span.record("request.id", request_id.as_str());
        }

        record_trace_context(&span);

        if let Some(protocol_version) = protocol_version {
            span.record("network.protocol.version", protocol_version);
        }

        let future = {
            let _entered = span.enter();
            self.service.call(request)
        };

        let response_span = span.clone();

        Box::pin(
            async move {
                let result = future.await;

                let (status_code, route) = match &result {
                    Ok(response) => (
                        response.status().as_u16(),
                        response.request().match_pattern(),
                    ),

                    Err(error) => (error.as_response_error().status_code().as_u16(), None),
                };

                finish_span(&response_span, &method, route.as_deref(), status_code);

                tracing::info!(
                    duration_ms = started_at.elapsed().as_millis(),
                    "HTTP request completed"
                );

                result
            }
            .instrument(span),
        )
    }
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|name| name.as_str()).collect()
    }
}

fn record_trace_context(span: &Span) {
    let context = span.context();

    let span_context = context.span().span_context().clone();

    if span_context.is_valid() {
        span.record("trace_id", span_context.trace_id().to_string());
        span.record("span_id", span_context.span_id().to_string());
    }
}

fn finish_span(span: &Span, method: &str, route: Option<&str>, status_code: u16) {
    span.record("http.response.status_code", i64::from(status_code));

    if let Some(route) = route {
        let span_name = format!("{method} {route}");

        span.record("http.route", route);
        span.record("otel.name", span_name.as_str());

        span.context().span().update_name(span_name);
    }

    if status_code >= 500 {
        let error_type = status_code.to_string();

        span.record("error.type", error_type.as_str());
        span.set_status(Status::error(format!("HTTP {status_code}")));
    }
}
