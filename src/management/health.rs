use std::time::Duration;

use actix_web::{HttpResponse, web};
use serde::Serialize;
use tokio::time;

use crate::app::ApplicationContext;

const READINESS_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Serialize)]
struct HealthResponse {
    status: HealthStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthStatus {
    Live,
    Ready,
    NotReady,
}

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    configuration
        .service(web::resource("/live").route(web::get().to(liveness)))
        .service(web::resource("/ready").route(web::get().to(readiness)));
}

async fn liveness() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse {
        status: HealthStatus::Live,
    })
}

async fn readiness(context: web::Data<ApplicationContext>) -> HttpResponse {
    match time::timeout(READINESS_TIMEOUT, context.check_readiness()).await {
        Ok(Ok(())) => HttpResponse::Ok().json(HealthResponse {
            status: HealthStatus::Ready,
        }),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "database readiness check failed");

            HttpResponse::ServiceUnavailable().json(HealthResponse {
                status: HealthStatus::NotReady,
            })
        }
        Err(_) => {
            tracing::warn!(
                timeout_seconds = READINESS_TIMEOUT.as_secs(),
                "database readiness check timed out"
            );

            HttpResponse::ServiceUnavailable().json(HealthResponse {
                status: HealthStatus::NotReady,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web};
    use serde_json::Value;

    use super::configure;

    #[actix_web::test]
    async fn liveness_reports_the_process_as_live() {
        let app =
            test::init_service(App::new().service(web::scope("/health").configure(configure)))
                .await;

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
}
