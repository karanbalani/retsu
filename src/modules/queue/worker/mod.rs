mod expired_message_cleaner;
mod visibility_timeout_processor;

pub(in crate::modules::queue) use expired_message_cleaner::{
    NAME as EXPIRED_MESSAGE_CLEANER_NAME, registration as expired_message_cleaner_registration,
};

pub(in crate::modules::queue) use visibility_timeout_processor::{
    NAME as VISIBILITY_TIMEOUT_PROCESSOR_NAME, registration as visibility_timeout_registration,
};
