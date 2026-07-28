mod message;
mod queue;

pub(in crate::modules::queue) use queue::{
    Queue, QueueConfigurationUpdate, QueueDetails, QueueNameError, QueueSettingsError,
    QueueValidationError,
};

pub(in crate::modules::queue) use message::{Message, MessagePriority, MessageValidationError};
