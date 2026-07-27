use actix_web::{App, http::StatusCode, test, web};
use serde_json::Value;

use super::configure;

#[actix_web::test]
async fn liveness_reports_the_process_as_live() {
    let app =
        test::init_service(App::new().service(web::scope("/health").configure(configure))).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/health/live").to_request(),
    )
    .await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "status": "live" }));
}
