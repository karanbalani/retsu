//! Composition root for vertical application modules.
//!
//! Infrastructure hosts depend only on this module. Individual
//! business modules remain private and expose registrations here.

mod queue;

use actix_web::web;
use thiserror::Error;

use crate::worker::WorkerRegistration;

pub(crate) use queue::QueueModule;

const AVAILABLE_WORKER_MODULES: &[&str] = &[queue::WORKER_MODULE];

/// Registers API routes contributed by business modules.
pub(crate) fn configure_api(configuration: &mut web::ServiceConfig) {
    queue::configure_api(configuration);
}

/// Resolves one named domain worker from its owning module.
///
/// Process infrastructure, such as the management listener, is
/// registered by the worker entrypoint rather than here.
pub(crate) fn worker_registration(
    module: &str,
    name: &str,
) -> Result<WorkerRegistration, WorkerSelectionError> {
    match module {
        queue::WORKER_MODULE => {
            queue::worker_registration(name).ok_or_else(|| WorkerSelectionError::UnknownWorker {
                module: module.to_owned(),
                name: name.to_owned(),
                available: queue::worker_names().join(", "),
            })
        }

        _ => Err(WorkerSelectionError::UnknownModule {
            module: module.to_owned(),
            available: AVAILABLE_WORKER_MODULES.join(", "),
        }),
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkerSelectionError {
    #[error("unknown worker module `{module}`; available modules: {available}")]
    UnknownModule { module: String, available: String },

    #[error(
        "unknown worker `{name}` in module `{module}`; \
         available workers: {available}"
    )]
    UnknownWorker {
        module: String,
        name: String,
        available: String,
    },
}
