//! Composition root for vertical application modules.
//!
//! Infrastructure hosts depend only on this module. Individual
//! business modules remain private and expose registrations here.

mod registration;

mod queue;

use actix_web::web;
use registration::{ModuleDescriptor, WorkerDescriptor};
use thiserror::Error;

use crate::worker::WorkerRegistration;

pub(crate) use queue::QueueModule;

const MODULES: &[ModuleDescriptor] = &[queue::DESCRIPTOR];

/// Registers API routes contributed by application modules.
pub(crate) fn configure_api(configuration: &mut web::ServiceConfig) {
    for module in MODULES {
        if let Some(configure_api) = module.api_configurer() {
            configure_api(configuration);
        }
    }
}

/// Returns modules that contribute at least one runnable worker.
pub(crate) fn worker_module_names() -> impl Iterator<Item = &'static str> {
    MODULES
        .iter()
        .filter(|module| !module.workers().is_empty())
        .map(ModuleDescriptor::name)
}

/// Returns the worker names owned by one worker-contributing module.
pub(crate) fn worker_names(
    module: &str,
) -> Result<impl Iterator<Item = &'static str>, WorkerSelectionError> {
    let module = worker_module_descriptor(module)?;

    Ok(module.workers().iter().map(WorkerDescriptor::name))
}

/// Resolves a CLI module and worker name into a canonical worker selection.
pub(crate) fn select_worker(
    module: &str,
    worker: &str,
) -> Result<SelectedWorker, WorkerSelectionError> {
    let module_descriptor = worker_module_descriptor(module)?;

    let worker_descriptor =
        module_descriptor
            .worker(worker)
            .ok_or_else(|| WorkerSelectionError::UnknownWorker {
                module: module.to_owned(),
                name: worker.to_owned(),
                available: display_names(
                    module_descriptor
                        .workers()
                        .iter()
                        .map(WorkerDescriptor::name),
                ),
            })?;

    Ok(SelectedWorker {
        module_name: module_descriptor.name(),
        worker_name: worker_descriptor.name(),
        registration: worker_descriptor.registration(),
    })
}

pub(crate) struct SelectedWorker {
    module_name: &'static str,
    worker_name: &'static str,
    registration: WorkerRegistration,
}

impl SelectedWorker {
    pub(crate) const fn module_name(&self) -> &'static str {
        self.module_name
    }

    pub(crate) const fn worker_name(&self) -> &'static str {
        self.worker_name
    }

    pub(crate) fn into_registration(self) -> WorkerRegistration {
        self.registration
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

fn worker_module_descriptor(name: &str) -> Result<&'static ModuleDescriptor, WorkerSelectionError> {
    MODULES
        .iter()
        .find(|module| module.name() == name && !module.workers().is_empty())
        .ok_or_else(|| WorkerSelectionError::UnknownModule {
            module: name.to_owned(),
            available: display_names(worker_module_names()),
        })
}

fn display_names(names: impl Iterator<Item = &'static str>) -> String {
    let names = names.collect::<Vec<_>>();

    if names.is_empty() {
        "<none>".to_owned()
    } else {
        names.join(", ")
    }
}
