use std::{borrow::Cow, error::Error as StdError, fmt};

use actix_web::{
    HttpResponse, ResponseError,
    body::BoxBody,
    http::{
        StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    detail: Cow<'static, str>,
    source: Option<anyhow::Error>,
}

impl ApiError {
    pub(crate) fn bad_request(code: &'static str, detail: impl Into<Cow<'static, str>>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, detail)
    }

    pub(crate) fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "the request body exceeds the allowed size",
        )
    }

    pub(crate) fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "the request content type must be JSON",
        )
    }

    pub(crate) fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "route_not_found",
            "the requested route was not found",
        )
    }

    pub(crate) fn internal(source: impl Into<anyhow::Error>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            detail: Cow::Borrowed("an unexpected error occurred"),
            source: Some(source.into()),
        }
    }

    pub(crate) fn conflict(code: &'static str, detail: impl Into<Cow<'static, str>>) -> Self {
        Self::new(StatusCode::CONFLICT, code, detail)
    }

    pub(crate) fn resource_not_found(
        code: &'static str,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, detail)
    }

    fn new(status: StatusCode, code: &'static str, detail: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            code,
            detail: detail.into(),
            source: None,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl StdError for ApiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn StdError + 'static))
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        if let Some(source) = &self.source {
            tracing::error!(error.code = self.code, error = %source, "request failed");
        }

        let body = ProblemDetails {
            kind: "about:blank",
            title: self.status.canonical_reason().unwrap_or("Error"),
            status: self.status.as_u16(),
            detail: &self.detail,
            code: self.code,
        };

        HttpResponse::build(self.status)
            .insert_header((CONTENT_TYPE, "application/problem+json"))
            .insert_header((CACHE_CONTROL, "no-store"))
            .json(body)
    }
}

#[derive(Serialize)]
struct ProblemDetails<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    title: &'static str,
    status: u16,
    detail: &'a str,
    code: &'static str,
}

pub(super) async fn not_found() -> Result<HttpResponse, ApiError> {
    Err(ApiError::not_found())
}

#[cfg(test)]
#[path = "../tests/api_error.rs"]
mod tests;
