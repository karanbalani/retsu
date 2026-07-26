//! Composition root for vertical application modules.
//!
//! Infrastructure hosts depend only on this module. Individual
//! business modules remain private and expose registrations here.

mod queue;

use actix_web::web;

use crate::worker::WorkerRegistration;

pub(crate) use queue::QueueModule;

/// Registers API routes contributed by business modules.
pub(crate) fn configure_api(configuration: &mut web::ServiceConfig) {
    queue::configure_api(configuration);
}

/// Returns long-running workers contributed by business modules.
///
/// Process infrastructure, such as worker management listener, is
/// registered by the worker entrypoint rather than here.
pub(crate) fn worker_registrations() -> Vec<WorkerRegistration> {
    vec![queue::worker_registration()]
}
