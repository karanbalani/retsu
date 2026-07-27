use actix_web::{
    App, HttpResponse,
    http::header::{REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS},
    test, web,
};

use super::default_response_headers;

#[actix_web::test]
async fn adds_safe_default_headers() {
    let app = test::init_service(
        App::new()
            .wrap(default_response_headers())
            .route("/", web::get().to(HttpResponse::Ok)),
    )
    .await;

    let response = test::call_service(&app, test::TestRequest::get().uri("/").to_request()).await;

    assert_eq!(
        response.headers().get(X_CONTENT_TYPE_OPTIONS),
        Some(&"nosniff".parse().expect("valid header value"))
    );
    assert_eq!(
        response.headers().get(REFERRER_POLICY),
        Some(&"no-referrer".parse().expect("valid header value"))
    );
}
