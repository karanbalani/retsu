use std::sync::{Arc, OnceLock};

use opentelemetry::metrics::Meter;

use super::{
    ExpiredMessageCleanerMetrics, QueueCommandMetrics, QueueStateMetrics, VisibilityTimeoutMetrics,
};

#[derive(Clone)]
pub(crate) struct QueueInstrumentation {
    inner: Arc<QueueInstrumentationInner>,
}

struct QueueInstrumentationInner {
    meter: Meter,
    commands: OnceLock<QueueCommandMetrics>,
    visibility_timeout: OnceLock<VisibilityTimeoutMetrics>,
    expired_message_cleaner: OnceLock<ExpiredMessageCleanerMetrics>,
    state: OnceLock<QueueStateMetrics>,
}

impl QueueInstrumentation {
    pub(super) fn new(meter: &Meter) -> Self {
        Self {
            inner: Arc::new(QueueInstrumentationInner {
                meter: meter.clone(),
                commands: OnceLock::new(),
                visibility_timeout: OnceLock::new(),
                expired_message_cleaner: OnceLock::new(),
                state: OnceLock::new(),
            }),
        }
    }

    pub(crate) fn commands(&self) -> QueueCommandMetrics {
        self.inner
            .commands
            .get_or_init(|| QueueCommandMetrics::new(&self.inner.meter))
            .clone()
    }

    pub(crate) fn visibility_timeout(&self) -> VisibilityTimeoutMetrics {
        self.inner
            .visibility_timeout
            .get_or_init(|| VisibilityTimeoutMetrics::new(&self.inner.meter))
            .clone()
    }

    pub(crate) fn expired_message_cleaner(&self) -> ExpiredMessageCleanerMetrics {
        self.inner
            .expired_message_cleaner
            .get_or_init(|| ExpiredMessageCleanerMetrics::new(&self.inner.meter))
            .clone()
    }

    pub(crate) fn state(&self) -> QueueStateMetrics {
        self.inner
            .state
            .get_or_init(|| QueueStateMetrics::new(&self.inner.meter))
            .clone()
    }
}
