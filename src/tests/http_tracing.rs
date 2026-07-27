use actix_web::{App, HttpResponse, http::StatusCode, test, web};
use opentelemetry::trace::{SpanKind, Status, TracerProvider as _};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
use uuid::Uuid;

use crate::http::{RequestIdMiddleware, tracing::HttpTracingMiddleware};

fn attribute(span: &SpanData, name: &str) -> Option<String> {
    span.attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == name)
        .map(|attribute| attribute.value.to_string())
}

#[actix_web::test]
async fn exports_bounded_server_spans_and_excludes_metrics() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let tracer = provider.tracer("retsu-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let guard = subscriber.set_default();

    let app = test::init_service(
        App::new()
            .wrap(HttpTracingMiddleware)
            .wrap(RequestIdMiddleware)
            .route(
                "/items/{identifier}",
                web::get().to(HttpResponse::NoContent),
            )
            .route("/failure", web::get().to(HttpResponse::ServiceUnavailable))
            .route("/metrics", web::get().to(HttpResponse::Ok)),
    )
    .await;

    let success =
        test::call_service(&app, test::TestRequest::get().uri("/items/42").to_request()).await;
    assert_eq!(success.status(), StatusCode::NO_CONTENT);

    let failure =
        test::call_service(&app, test::TestRequest::get().uri("/failure").to_request()).await;
    assert_eq!(failure.status(), StatusCode::SERVICE_UNAVAILABLE);

    let metrics =
        test::call_service(&app, test::TestRequest::get().uri("/metrics").to_request()).await;
    assert_eq!(metrics.status(), StatusCode::OK);

    provider.force_flush().expect("spans should flush");

    let spans = exporter
        .get_finished_spans()
        .expect("finished spans should be readable");

    assert_eq!(spans.len(), 2);

    let success_span = spans
        .iter()
        .find(|span| span.name == "GET /items/{identifier}")
        .expect("matched route span should exist");

    assert_eq!(success_span.span_kind, SpanKind::Server);
    assert_eq!(success_span.status, Status::Unset);
    assert_eq!(
        attribute(success_span, "http.request.method").as_deref(),
        Some("GET")
    );
    assert_eq!(
        attribute(success_span, "url.scheme").as_deref(),
        Some("http")
    );
    assert_eq!(
        attribute(success_span, "network.protocol.version").as_deref(),
        Some("1.1")
    );
    assert_eq!(
        attribute(success_span, "http.route").as_deref(),
        Some("/items/{identifier}")
    );
    assert_eq!(
        attribute(success_span, "http.response.status_code").as_deref(),
        Some("204")
    );
    assert!(
        attribute(success_span, "request.id")
            .is_some_and(|request_id| Uuid::parse_str(&request_id).is_ok())
    );

    let failure_span = spans
        .iter()
        .find(|span| span.name == "GET /failure")
        .expect("failure span should exist");

    assert_eq!(failure_span.span_kind, SpanKind::Server);
    assert_eq!(failure_span.status, Status::error("HTTP 503"));
    assert_eq!(
        attribute(failure_span, "error.type").as_deref(),
        Some("503")
    );
    assert_eq!(
        attribute(failure_span, "http.response.status_code").as_deref(),
        Some("503")
    );
    assert!(!spans.iter().any(|span| span.name.contains("metrics")));

    drop(app);
    drop(guard);
    provider.shutdown().expect("provider should shut down");
}
