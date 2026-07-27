use actix_web::web;

use crate::worker::WorkerRegistration;

pub(super) type ApiConfigurer = fn(&mut web::ServiceConfig);

pub(super) struct ModuleDefinition {
    name: &'static str,
    api_configurer: Option<ApiConfigurer>,
    workers: &'static [WorkerDefinition],
}

impl ModuleDefinition {
    pub(super) const fn new(name: &'static str) -> Self {
        Self {
            name,
            api_configurer: None,
            workers: &[],
        }
    }

    pub(super) const fn with_api(mut self, api_configurer: ApiConfigurer) -> Self {
        self.api_configurer = Some(api_configurer);
        self
    }

    pub(super) const fn with_workers(mut self, workers: &'static [WorkerDefinition]) -> Self {
        self.workers = workers;
        self
    }

    pub(super) const fn name(&self) -> &'static str {
        self.name
    }

    pub(super) const fn api_configurer(&self) -> Option<ApiConfigurer> {
        self.api_configurer
    }

    pub(super) const fn workers(&self) -> &'static [WorkerDefinition] {
        self.workers
    }

    pub(super) fn worker(&self, name: &str) -> Option<&WorkerDefinition> {
        self.workers.iter().find(|worker| worker.name() == name)
    }
}

pub(super) struct WorkerDefinition {
    name: &'static str,
    registration_factory: fn() -> WorkerRegistration,
}

impl WorkerDefinition {
    pub(super) const fn new(
        name: &'static str,
        registration_factory: fn() -> WorkerRegistration,
    ) -> Self {
        Self {
            name,
            registration_factory,
        }
    }

    pub(super) const fn name(&self) -> &'static str {
        self.name
    }

    pub(super) fn build_registration(&self) -> WorkerRegistration {
        (self.registration_factory)()
    }
}
