mod dead_letter_message_cleaner;
mod expired_message_cleaner;
mod state_metrics_collector;

pub(in crate::modules::queue) use dead_letter_message_cleaner::{
    NAME as DEAD_LETTER_MESSAGE_CLEANER_NAME,
    registration as dead_letter_message_cleaner_registration,
};

pub(in crate::modules::queue) use expired_message_cleaner::{
    NAME as EXPIRED_MESSAGE_CLEANER_NAME, registration as expired_message_cleaner_registration,
};

pub(in crate::modules::queue) use state_metrics_collector::{
    NAME as STATE_METRICS_COLLECTOR_NAME, registration as state_metrics_collector_registration,
};
