use actix_web::web;

use crate::worker::WorkerRegistration;

pub(super) type ApiConfigurer = fn(&mut web::ServiceConfig);

pub(super) struct ModuleDescriptor {
    name: &'static str,
    api_configurer: Option<ApiConfigurer>,
    workers: &'static [WorkerDescriptor],
}

impl ModuleDescriptor {
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

    pub(super) const fn with_workers(mut self, workers: &'static [WorkerDescriptor]) -> Self {
        self.workers = workers;
        self
    }

    pub(super) const fn name(&self) -> &'static str {
        self.name
    }

    pub(super) const fn api_configurer(&self) -> Option<ApiConfigurer> {
        self.api_configurer
    }

    pub(super) const fn workers(&self) -> &'static [WorkerDescriptor] {
        self.workers
    }

    pub(super) fn worker(&self, name: &str) -> Option<&WorkerDescriptor> {
        self.workers.iter().find(|worker| worker.name() == name)
    }
}

pub(super) struct WorkerDescriptor {
    name: &'static str,
    registration: fn() -> WorkerRegistration,
}

impl WorkerDescriptor {
    pub(super) const fn new(name: &'static str, registration: fn() -> WorkerRegistration) -> Self {
        Self { name, registration }
    }

    pub(super) const fn name(&self) -> &'static str {
        self.name
    }

    pub(super) fn registration(&self) -> WorkerRegistration {
        (self.registration)()
    }
}
