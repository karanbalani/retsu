use std::{
    fmt,
    future::{Ready, ready},
    pin::Pin,
};

use actix_web::{
    Error, HttpMessage,
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{HeaderName, HeaderValue},
};
use uuid::Uuid;

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const MAX_REQUEST_ID_LENGTH: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct RequestId(String);

impl RequestId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(crate) struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdService { service }))
    }
}

pub(crate) struct RequestIdService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdService<S>
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
        let request_id = request_id(&request);

        let response_header = HeaderValue::from_str(request_id.as_str())
            .expect("validated or generated request IDs are valid headers");

        request.extensions_mut().insert(request_id);

        let future = self.service.call(request);

        Box::pin(async move {
            let mut response = future.await?;

            response
                .headers_mut()
                .insert(REQUEST_ID_HEADER, response_header);

            Ok(response)
        })
    }
}

fn request_id(request: &ServiceRequest) -> RequestId {
    request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_request_id(value))
        .map(|value| RequestId(value.to_owned()))
        .unwrap_or_else(|| RequestId(Uuid::new_v4().to_string()))
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use actix_web::{
        App, HttpMessage as _, HttpRequest, HttpResponse, http::header::HeaderValue, test, web,
    };
    use uuid::Uuid;

    use super::{
        MAX_REQUEST_ID_LENGTH, REQUEST_ID_HEADER, RequestId, RequestIdMiddleware, valid_request_id,
    };

    async fn echo_request_id(request: HttpRequest) -> HttpResponse {
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .expect("request ID should be available to handlers")
            .to_string();

        HttpResponse::Ok().body(request_id)
    }

    #[actix_web::test]
    async fn preserves_a_valid_inbound_request_id() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(echo_request_id)),
        )
        .await;
        let request = test::TestRequest::get()
            .uri("/")
            .insert_header((REQUEST_ID_HEADER, "client-id_1.2:3"))
            .to_request();

        let response = test::call_service(&app, request).await;
        let response_header = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .expect("response should contain request ID")
            .to_str()
            .expect("request ID should be text")
            .to_owned();
        let body = test::read_body(response).await;

        assert_eq!(response_header, "client-id_1.2:3");
        assert_eq!(body, "client-id_1.2:3");
    }

    #[actix_web::test]
    async fn generates_a_uuid_when_the_request_id_is_missing() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(echo_request_id)),
        )
        .await;
        let request = test::TestRequest::get().uri("/").to_request();

        let response = test::call_service(&app, request).await;
        let response_header = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .expect("response should contain request ID")
            .to_str()
            .expect("request ID should be text")
            .to_owned();
        let body = test::read_body(response).await;

        assert!(Uuid::parse_str(&response_header).is_ok());
        assert_eq!(body, response_header);
    }

    #[actix_web::test]
    async fn replaces_invalid_inbound_request_ids() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(echo_request_id)),
        )
        .await;
        let invalid_values = [
            HeaderValue::from_static("contains space"),
            HeaderValue::from_str(&"a".repeat(MAX_REQUEST_ID_LENGTH + 1))
                .expect("long value should still be a valid header"),
        ];

        for invalid_value in invalid_values {
            let request = test::TestRequest::get()
                .uri("/")
                .insert_header((REQUEST_ID_HEADER, invalid_value))
                .to_request();

            let response = test::call_service(&app, request).await;
            let generated = response
                .headers()
                .get(&REQUEST_ID_HEADER)
                .expect("response should contain request ID")
                .to_str()
                .expect("request ID should be text");

            assert!(Uuid::parse_str(generated).is_ok());
        }
    }

    #[actix_web::test]
    async fn validates_request_id_length_and_character_boundaries() {
        assert!(valid_request_id(&"a".repeat(MAX_REQUEST_ID_LENGTH)));
        assert!(valid_request_id("letters-123_under.score:segment"));
        assert!(!valid_request_id(""));
        assert!(!valid_request_id(&"a".repeat(MAX_REQUEST_ID_LENGTH + 1)));
        assert!(!valid_request_id("contains space"));
        assert!(!valid_request_id("non-ascii-ß"));
    }
}
