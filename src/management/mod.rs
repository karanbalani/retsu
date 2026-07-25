mod health;
mod metrics;

use actix_web::web;

pub(crate) fn configure(configuration: &mut web::ServiceConfig) {
    configuration.service(web::scope("/health").configure(health::configure));

    metrics::configure(configuration);
}
