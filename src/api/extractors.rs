use actix_web::{
    error::{JsonPayloadError, PathError, QueryPayloadError},
    web,
};

use super::ApiError;

const JSON_BODY_LIMIT_BYTES: usize = 1024 * 1024; // 1MB

pub(super) fn json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .limit(JSON_BODY_LIMIT_BYTES)
        .content_type_required(true)
        .error_handler(|error, _request| json_error(error).into())
}

pub(super) fn path_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|error, _request| path_error(error).into())
}

pub(super) fn query_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|error, _request| query_error(error).into())
}

fn json_error(error: JsonPayloadError) -> ApiError {
    match error {
        JsonPayloadError::OverflowKnownLength { .. } | JsonPayloadError::Overflow { .. } => {
            ApiError::payload_too_large()
        }

        JsonPayloadError::ContentType => ApiError::unsupported_media_type(),

        JsonPayloadError::Deserialize(_) | JsonPayloadError::Payload(_) => {
            ApiError::bad_request("invalid_json", "the request body contains invalid JSON")
        }

        JsonPayloadError::Serialize(error) => ApiError::internal(error),

        error => ApiError::internal(anyhow::anyhow!("unhandled JSON payload error: {error}")),
    }
}

fn path_error(_error: PathError) -> ApiError {
    ApiError::bad_request("invalid_path", "the request path contains an invalid value")
}

fn query_error(_error: QueryPayloadError) -> ApiError {
    ApiError::bad_request(
        "invalid_query",
        "the request query contains an invalid value",
    )
}

#[cfg(test)]
mod tests {
    use actix_web::{
        App, HttpResponse,
        http::{StatusCode, header::CONTENT_TYPE},
        test, web,
    };
    use serde::Deserialize;
    use serde_json::Value;

    use super::{JSON_BODY_LIMIT_BYTES, json_config, path_config, query_config};

    #[derive(Deserialize)]
    struct JsonBody {
        value: String,
    }

    #[derive(Deserialize)]
    struct Query {
        limit: u32,
    }

    async fn json(body: web::Json<JsonBody>) -> HttpResponse {
        HttpResponse::Ok().body(body.value.clone())
    }

    async fn path(identifier: web::Path<u64>) -> HttpResponse {
        HttpResponse::Ok().body(identifier.to_string())
    }

    async fn query(query: web::Query<Query>) -> HttpResponse {
        HttpResponse::Ok().body(query.limit.to_string())
    }

    #[actix_web::test]
    async fn maps_missing_json_content_type_to_problem_details() {
        let app = test::init_service(
            App::new()
                .app_data(json_config())
                .route("/json", web::post().to(json)),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/json")
            .set_payload(r#"{"value":"ignored"}"#)
            .to_request();

        let response = test::call_service(&app, request).await;
        let status = response.status();
        let content_type = response.headers().get(CONTENT_TYPE).cloned();
        let body: Value = test::read_body_json(response).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            content_type,
            Some(
                "application/problem+json"
                    .parse()
                    .expect("valid content type")
            )
        );
        assert_eq!(body["code"], "unsupported_media_type");
    }

    #[actix_web::test]
    async fn maps_malformed_json_to_problem_details() {
        let app = test::init_service(
            App::new()
                .app_data(json_config())
                .route("/json", web::post().to(json)),
        )
        .await;
        let request = test::TestRequest::post()
            .uri("/json")
            .insert_header((CONTENT_TYPE, "application/json"))
            .set_payload(r#"{"value":"unfinished"#)
            .to_request();

        let response = test::call_service(&app, request).await;
        let status = response.status();
        let body: Value = test::read_body_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "invalid_json");
    }

    #[actix_web::test]
    async fn rejects_json_larger_than_the_configured_limit() {
        let app = test::init_service(
            App::new()
                .app_data(json_config())
                .route("/json", web::post().to(json)),
        )
        .await;
        let payload = serde_json::json!({
            "value": "a".repeat(JSON_BODY_LIMIT_BYTES)
        })
        .to_string();
        let request = test::TestRequest::post()
            .uri("/json")
            .insert_header((CONTENT_TYPE, "application/json"))
            .set_payload(payload)
            .to_request();

        let response = test::call_service(&app, request).await;
        let status = response.status();
        let body: Value = test::read_body_json(response).await;

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["code"], "payload_too_large");
    }

    #[actix_web::test]
    async fn maps_invalid_path_and_query_values_to_problem_details() {
        let app = test::init_service(
            App::new()
                .app_data(path_config())
                .app_data(query_config())
                .route("/path/{identifier}", web::get().to(path))
                .route("/query", web::get().to(query)),
        )
        .await;

        let path_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/path/not-a-number")
                .to_request(),
        )
        .await;
        let path_status = path_response.status();
        let path_body: Value = test::read_body_json(path_response).await;

        let query_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/query?limit=not-a-number")
                .to_request(),
        )
        .await;
        let query_status = query_response.status();
        let query_body: Value = test::read_body_json(query_response).await;

        assert_eq!(path_status, StatusCode::BAD_REQUEST);
        assert_eq!(path_body["code"], "invalid_path");
        assert_eq!(query_status, StatusCode::BAD_REQUEST);
        assert_eq!(query_body["code"], "invalid_query");
    }
}
