use std::error::Error as _;

use actix_web::{
    ResponseError as _,
    body::to_bytes,
    http::{
        StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
};
use serde_json::Value;

use super::ApiError;

async fn problem_details(error: ApiError) -> Value {
    let response = error.error_response();

    assert_eq!(
        response.headers().get(CONTENT_TYPE),
        Some(
            &"application/problem+json"
                .parse()
                .expect("valid content type")
        )
    );
    assert_eq!(
        response.headers().get(CACHE_CONTROL),
        Some(&"no-store".parse().expect("valid cache control"))
    );

    let body = to_bytes(response.into_body())
        .await
        .expect("problem body should be readable");

    serde_json::from_slice(&body).expect("problem body should be JSON")
}

#[actix_web::test]
async fn serializes_stable_problem_details() {
    let body = problem_details(ApiError::bad_request(
        "invalid_input",
        "the supplied value is invalid",
    ))
    .await;

    assert_eq!(body["type"], "about:blank");
    assert_eq!(body["title"], "Bad Request");
    assert_eq!(body["status"], 400);
    assert_eq!(body["detail"], "the supplied value is invalid");
    assert_eq!(body["code"], "invalid_input");
}

#[actix_web::test]
async fn internal_errors_hide_their_source_from_clients() {
    let error = ApiError::internal(anyhow::anyhow!("database password was rejected"));

    assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        error.source().map(ToString::to_string),
        Some("database password was rejected".to_owned())
    );

    let body = problem_details(error).await;

    assert_eq!(body["status"], 500);
    assert_eq!(body["code"], "internal_error");
    assert_eq!(body["detail"], "an unexpected error occurred");
    assert!(!body.to_string().contains("database password was rejected"));
}
