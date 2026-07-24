mod health;

use actix_web::web;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    configuration.service(web::scope("/health").configure(health::configure));
}
