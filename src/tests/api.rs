use actix_web::{
    App, HttpResponse,
    http::{Method, StatusCode, header::ALLOW},
    test, web,
};
use serde_json::Value;

use super::error;

#[actix_web::test]
async fn unknown_routes_use_the_api_problem_contract() {
    let app = test::init_service(App::new().default_service(web::to(error::not_found))).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/not-a-route").to_request(),
    )
    .await;
    let status = response.status();
    let body: Value = test::read_body_json(response).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "route_not_found");
}

#[actix_web::test]
async fn known_routes_preserve_actix_method_not_allowed_responses() {
    let app = test::init_service(
        App::new()
            .service(web::resource("/known").route(web::get().to(HttpResponse::NoContent)))
            .default_service(web::to(error::not_found)),
    )
    .await;
    let request = test::TestRequest::default()
        .method(Method::POST)
        .uri("/known")
        .to_request();

    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response.headers().get(ALLOW),
        Some(&"GET".parse().expect("valid Allow header"))
    );
}
