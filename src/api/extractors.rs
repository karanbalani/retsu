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
#[path = "../tests/api_extractors.rs"]
mod tests;
