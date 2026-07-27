mod acknowledge_message;
mod create_queue;
mod dequeue_message;
mod enqueue_message;
mod process_timed_out_messages;
mod repository;

pub(in crate::modules::queue) use create_queue::{
    CreateQueueCommand, CreateQueueError, CreatedQueue, execute as execute_create_queue,
};

pub(in crate::modules::queue) use repository::{
    AcknowledgeMessageOutcome, CreateQueueOutcome, DequeueMessageOutcome, EnqueueMessageOutcome,
    MessageRepository, QueueRepository, QueueTimeoutProcessingSummary, TimeoutProcessingSummary,
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

#[cfg(test)]
mod lifecycle_tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    };

    use uuid::Uuid;

    use super::{
        AcknowledgeMessageCommand, AcknowledgeMessageError, AcknowledgeMessageOutcome,
        DequeueMessageOutcome, EnqueueMessageOutcome, MessageRepository,
        QueueTimeoutProcessingSummary, TimeoutProcessingSummary, execute_acknowledge_message,
        execute_process_timed_out_messages,
    };
    use crate::modules::queue::domain::Message;

    struct FakeMessageRepository {
        acknowledge_outcome: AcknowledgeMessageOutcome,
        acknowledge_call: Mutex<Option<(String, Uuid, Uuid)>>,
        timeout_requeued: u64,
        timeout_dead_lettered: u64,
        timeout_batch_size: AtomicU32,
    }

    impl FakeMessageRepository {
        fn with_acknowledge_outcome(outcome: AcknowledgeMessageOutcome) -> Self {
            Self {
                acknowledge_outcome: outcome,
                acknowledge_call: Mutex::new(None),
                timeout_requeued: 0,
                timeout_dead_lettered: 0,
                timeout_batch_size: AtomicU32::new(0),
            }
        }

        fn with_timeout_counts(requeued: u64, dead_lettered: u64) -> Self {
            Self {
                acknowledge_outcome: AcknowledgeMessageOutcome::Acknowledged,
                acknowledge_call: Mutex::new(None),
                timeout_requeued: requeued,
                timeout_dead_lettered: dead_lettered,
                timeout_batch_size: AtomicU32::new(0),
            }
        }
    }

    impl MessageRepository for FakeMessageRepository {
        async fn enqueue_message(
            &self,
            _queue_name: &str,
            _message: &Message,
        ) -> Result<EnqueueMessageOutcome, anyhow::Error> {
            unreachable!("lifecycle tests should not enqueue messages")
        }

        async fn dequeue_message(
            &self,
            _queue_name: &str,
            _receipt_handle: Uuid,
        ) -> Result<DequeueMessageOutcome, anyhow::Error> {
            unreachable!("lifecycle tests should not dequeue messages")
        }

        async fn acknowledge_message(
            &self,
            queue_name: &str,
            message_id: Uuid,
            receipt_handle: Uuid,
        ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
            self.acknowledge_call
                .lock()
                .expect("acknowledgement call lock should not be poisoned")
                .replace((queue_name.to_owned(), message_id, receipt_handle));

            Ok(match self.acknowledge_outcome {
                AcknowledgeMessageOutcome::Acknowledged => AcknowledgeMessageOutcome::Acknowledged,
                AcknowledgeMessageOutcome::QueueNotFound => {
                    AcknowledgeMessageOutcome::QueueNotFound
                }
                AcknowledgeMessageOutcome::MessageNotFound => {
                    AcknowledgeMessageOutcome::MessageNotFound
                }
                AcknowledgeMessageOutcome::ReceiptHandleInvalid => {
                    AcknowledgeMessageOutcome::ReceiptHandleInvalid
                }
            })
        }

        async fn process_timed_out_messages(
            &self,
            batch_size: u32,
        ) -> Result<TimeoutProcessingSummary, anyhow::Error> {
            self.timeout_batch_size.store(batch_size, Ordering::Relaxed);

            Ok(TimeoutProcessingSummary::new(vec![
                QueueTimeoutProcessingSummary::new(
                    "email-delivery".to_owned(),
                    self.timeout_requeued,
                    self.timeout_dead_lettered,
                ),
            ]))
        }
    }

    #[tokio::test]
    async fn acknowledges_the_exact_message_delivery() {
        let repository = FakeMessageRepository::with_acknowledge_outcome(
            AcknowledgeMessageOutcome::Acknowledged,
        );
        let message_id = Uuid::now_v7();
        let receipt_handle = Uuid::new_v4();

        execute_acknowledge_message(
            &repository,
            AcknowledgeMessageCommand::new("email-delivery".to_owned(), message_id, receipt_handle),
        )
        .await
        .expect("current message delivery should be acknowledged");

        let call = repository
            .acknowledge_call
            .lock()
            .expect("acknowledgement call lock should not be poisoned");
        let (queue_name, actual_message_id, actual_receipt_handle) = call
            .as_ref()
            .expect("repository should receive the acknowledgement");

        assert_eq!(queue_name, "email-delivery");
        assert_eq!(*actual_message_id, message_id);
        assert_eq!(*actual_receipt_handle, receipt_handle);
    }

    #[tokio::test]
    async fn preserves_distinct_acknowledgement_failures() {
        async fn result_for(
            outcome: AcknowledgeMessageOutcome,
        ) -> Result<(), AcknowledgeMessageError> {
            let repository = FakeMessageRepository::with_acknowledge_outcome(outcome);

            execute_acknowledge_message(
                &repository,
                AcknowledgeMessageCommand::new(
                    "email-delivery".to_owned(),
                    Uuid::now_v7(),
                    Uuid::new_v4(),
                ),
            )
            .await
        }

        assert!(matches!(
            result_for(AcknowledgeMessageOutcome::QueueNotFound).await,
            Err(AcknowledgeMessageError::QueueNotFound)
        ));
        assert!(matches!(
            result_for(AcknowledgeMessageOutcome::MessageNotFound).await,
            Err(AcknowledgeMessageError::MessageNotFound)
        ));
        assert!(matches!(
            result_for(AcknowledgeMessageOutcome::ReceiptHandleInvalid).await,
            Err(AcknowledgeMessageError::ReceiptHandleInvalid)
        ));
    }

    #[tokio::test]
    async fn forwards_timeout_processing_and_preserves_the_summary() {
        let repository = FakeMessageRepository::with_timeout_counts(37, 5);

        let summary = execute_process_timed_out_messages(&repository, 500)
            .await
            .expect("timed-out messages should be processed");

        assert_eq!(repository.timeout_batch_size.load(Ordering::Relaxed), 500);
        assert_eq!(summary.processed(), 42);
        assert_eq!(summary.requeued(), 37);
        assert_eq!(summary.dead_lettered(), 5);

        let queue_summary = summary
            .per_queue()
            .first()
            .expect("summary should contain the affected queue");

        assert_eq!(queue_summary.queue_name(), "email-delivery");
        assert_eq!(queue_summary.requeued(), 37);
        assert_eq!(queue_summary.dead_lettered(), 5);
    }
}
