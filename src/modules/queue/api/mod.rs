mod dto;
mod handlers;

use actix_web::web;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    configuration.service(
        web::scope("/v1/queues")
            .route("", web::post().to(handlers::create_queue))
            .route("/{queue_id}", web::patch().to(handlers::update_queue))
            .route(
                "/{queue_id}/messages",
                web::post().to(handlers::enqueue_message),
            )
            .route(
                "/{queue_id}/messages/dequeue",
                web::post().to(handlers::dequeue_message),
            )
            .route(
                "/{queue_id}/messages/{message_id}/acknowledge",
                web::post().to(handlers::acknowledge_message),
            ),
    );
}
