use actix_web::web;

pub(super) fn configure(configuration: &mut web::ServiceConfig) {
    // Platform owned operational endpoints
    crate::management::configure(configuration);

    // Business endpoints contributed by vertical modules.
    crate::modules::configure_api(configuration);
}
