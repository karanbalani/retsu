use actix_web::{HttpResponse, error::ErrorInternalServerError, http::header::CONTENT_TYPE, web};

use crate::app::ApplicationContext;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    configuration.route("/metrics", web::get().to(scrape));
}

async fn scrape(context: web::Data<ApplicationContext>) -> actix_web::Result<HttpResponse> {
    let body = context.metrics().encode_prometheus().map_err(|error| {
        tracing::error!(error = %error, "failed to encode Prometheus metrics");
        ErrorInternalServerError("failed to encode metrics")
    })?;

    Ok(HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, prometheus::TEXT_FORMAT))
        .body(body))
}
