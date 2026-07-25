//! Composition root for vertical application modules.
//!
//! Infrastructure hosts depend only on this module. Individual
//! business modules remain private and expose registrations here.

use actix_web::web;

use crate::worker::WorkerRegistration;

/// Registers API routes contributed by business modules.
///
/// This stays empty until the first vertical module is added.
pub(crate) fn configure_api(_configuration: &mut web::ServiceConfig) {}

/// Returns long-running workers contributed by business modules.
///
/// Process infrastructure, such as worker management listener, is
/// registered by the worker entrypoint rather than here.
pub(crate) fn worker_registrations() -> Vec<WorkerRegistration> {
    Vec::new()
}
