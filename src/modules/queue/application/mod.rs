mod acknowledge_message;
mod create_queue;
mod dequeue_message;
mod enqueue_message;
mod process_expired_messages;
mod process_timed_out_messages;
mod repository;

pub(in crate::modules::queue) use create_queue::{
    CreateQueueCommand, CreateQueueError, CreatedQueue, execute as execute_create_queue,
};

pub(in crate::modules::queue) use repository::{
    AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome, EnqueueMessageOutcome,
    ExpiredMessagesCleanupSummary, MessageRepository, QueueExpiredMessagesCleanupSummary,
    QueuePriorityStateSnapshot, QueueRepository, QueueStateRepository,
    QueueTimeoutProcessingSummary, TimeoutProcessingSummary,
};

pub(in crate::modules::queue) use dequeue_message::{
    DequeueMessageCommand, DequeueMessageError, DequeuedMessage, execute as execute_dequeue_message,
};

pub(in crate::modules::queue) use enqueue_message::{
    EnqueueMessageCommand, EnqueueMessageError, EnqueuedMessage, execute as execute_enqueue_message,
};

pub(in crate::modules::queue) use acknowledge_message::{
    AcknowledgeMessageCommand, AcknowledgeMessageError, execute as execute_acknowledge_message,
};

pub(in crate::modules::queue) use process_timed_out_messages::{
    ProcessTimedOutMessagesError, execute as execute_process_timed_out_messages,
};

pub(in crate::modules::queue) use process_expired_messages::{
    ProcessExpiredMessagesError, execute as execute_process_expired_messages,
};

#[cfg(test)]
#[path = "../tests/application_lifecycle.rs"]
mod lifecycle_tests;
