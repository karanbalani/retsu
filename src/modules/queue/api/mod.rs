mod dto;
mod handlers;

use actix_web::web;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    configuration
        .service(web::scope("/v1/queues").route("", web::post().to(handlers::create_queue)));
}
