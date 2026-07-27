use actix_web::{
    http::header::{REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS},
    middleware::DefaultHeaders,
};

pub(crate) fn default_response_headers() -> DefaultHeaders {
    DefaultHeaders::new()
        .add((X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .add((REFERRER_POLICY, "no-referrer"))
}

#[cfg(test)]
#[path = "../tests/http_headers.rs"]
mod tests;
