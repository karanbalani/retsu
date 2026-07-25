use actix_web::web;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    crate::management::configure(configuration);
}
