//! Composition root for vertical application modules.
//!
//! Infrastructure hosts depend only on this module. Individual
//! business modules remain private and expose registrations here.

mod definition;

mod queue;

use actix_web::web;
use definition::{ModuleDefinition, WorkerDefinition};
use thiserror::Error;

use crate::{configuration::WorkerConfig, worker::WorkerRegistration};

pub(crate) use queue::QueueModule;

const MODULE_CATALOG: &[ModuleDefinition] = &[queue::DEFINITION];

/// Registers API routes contributed by application modules.
pub(crate) fn configure_api(configuration: &mut web::ServiceConfig) {
    for module in MODULE_CATALOG {
        if let Some(configure_api) = module.api_configurer() {
            configure_api(configuration);
        }
    }
}

/// Returns modules that contribute at least one runnable worker.
pub(crate) fn worker_module_names() -> impl Iterator<Item = &'static str> {
    MODULE_CATALOG
        .iter()
        .filter(|module| !module.workers().is_empty())
        .map(ModuleDefinition::name)
}

/// Returns the worker names owned by one worker-contributing module.
pub(crate) fn worker_names(
    module: &str,
) -> Result<impl Iterator<Item = &'static str>, WorkerResolutionError> {
    let module = worker_module_definition(module)?;

    Ok(module.workers().iter().map(WorkerDefinition::name))
}

/// Resolves a CLI module and worker name into a canonical worker selection.
pub(crate) fn resolve_worker(
    module: &str,
    worker: &str,
) -> Result<ResolvedWorker, WorkerResolutionError> {
    let module_definition = worker_module_definition(module)?;

    let worker_definition =
        module_definition
            .worker(worker)
            .ok_or_else(|| WorkerResolutionError::UnknownWorker {
                module: module.to_owned(),
                name: worker.to_owned(),
                available: display_names(
                    module_definition
                        .workers()
                        .iter()
                        .map(WorkerDefinition::name),
                ),
            })?;

    Ok(ResolvedWorker {
        module_name: module_definition.name(),
        worker_name: worker_definition.name(),
        definition: worker_definition,
    })
}

pub(crate) struct ResolvedWorker {
    module_name: &'static str,
    worker_name: &'static str,
    definition: &'static WorkerDefinition,
}

impl ResolvedWorker {
    pub(crate) const fn module_name(&self) -> &'static str {
        self.module_name
    }

    pub(crate) const fn worker_name(&self) -> &'static str {
        self.worker_name
    }

    pub(crate) fn build_registration(self, configuration: &WorkerConfig) -> WorkerRegistration {
        self.definition.build_registration(configuration)
    }

    #[cfg(test)]
    pub(crate) fn into_registration(self) -> WorkerRegistration {
        self.build_registration(&WorkerConfig::default())
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkerResolutionError {
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

fn worker_module_definition(
    name: &str,
) -> Result<&'static ModuleDefinition, WorkerResolutionError> {
    MODULE_CATALOG
        .iter()
        .find(|module| module.name() == name && !module.workers().is_empty())
        .ok_or_else(|| WorkerResolutionError::UnknownModule {
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
