use std::sync::{Arc, OnceLock};

use opentelemetry::metrics::Meter;

use super::{
    ExpiredMessageCleanerMetrics, QueueCommandMetrics, QueueStateMetrics, VisibilityTimeoutMetrics,
};

#[derive(Clone)]
pub(crate) struct QueueInstrumentation {
    meter: Meter,
    commands: Arc<OnceLock<QueueCommandMetrics>>,
    visibility_timeout: Arc<OnceLock<VisibilityTimeoutMetrics>>,
    expired_message_cleaner: Arc<OnceLock<ExpiredMessageCleanerMetrics>>,
    state: Arc<OnceLock<QueueStateMetrics>>,
}

impl QueueInstrumentation {
    pub(super) fn new(meter: &Meter) -> Self {
        Self {
            meter: meter.clone(),
            commands: Arc::new(OnceLock::new()),
            visibility_timeout: Arc::new(OnceLock::new()),
            expired_message_cleaner: Arc::new(OnceLock::new()),
            state: Arc::new(OnceLock::new()),
        }
    }

    pub(crate) fn commands(&self) -> QueueCommandMetrics {
        self.commands
            .get_or_init(|| QueueCommandMetrics::new(&self.meter))
            .clone()
    }

    pub(crate) fn visibility_timeout(&self) -> VisibilityTimeoutMetrics {
        self.visibility_timeout
            .get_or_init(|| VisibilityTimeoutMetrics::new(&self.meter))
            .clone()
    }

    pub(crate) fn expired_message_cleaner(&self) -> ExpiredMessageCleanerMetrics {
        self.expired_message_cleaner
            .get_or_init(|| ExpiredMessageCleanerMetrics::new(&self.meter))
            .clone()
    }

    pub(crate) fn state(&self) -> QueueStateMetrics {
        self.state
            .get_or_init(|| QueueStateMetrics::new(&self.meter))
            .clone()
    }
}
