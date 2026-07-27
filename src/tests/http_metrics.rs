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
