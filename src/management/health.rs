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
