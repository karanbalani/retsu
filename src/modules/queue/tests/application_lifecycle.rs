use std::sync::atomic::{AtomicU32, Ordering};

use uuid::Uuid;

use super::{
    AcknowledgeMessageCommand, AcknowledgeMessageError, AcknowledgeMessageOutcome,
    DequeueMessageOutcome, EnqueueMessageOutcome, ExpiredMessagesCleanupSummary, MessageRepository,
    QueueExpiredMessagesCleanupSummary, QueueTimeoutProcessingSummary, TimeoutProcessingSummary,
    execute_acknowledge_message, execute_process_expired_messages,
    execute_process_timed_out_messages,
};
use crate::modules::queue::domain::Message;

struct FakeMessageRepository {
    acknowledge_outcome: AcknowledgeMessageOutcome,
    timeout_requeued: u64,
    timeout_dead_lettered: u64,
    timeout_batch_size: AtomicU32,
    expired_never_delivered: u64,
    expired_previously_delivered: u64,
    expiration_batch_size: AtomicU32,
}

impl FakeMessageRepository {
    fn with_acknowledge_outcome(outcome: AcknowledgeMessageOutcome) -> Self {
        Self {
            acknowledge_outcome: outcome,
            timeout_requeued: 0,
            timeout_dead_lettered: 0,
            timeout_batch_size: AtomicU32::new(0),
            expired_never_delivered: 0,
            expired_previously_delivered: 0,
            expiration_batch_size: AtomicU32::new(0),
        }
    }

    fn with_timeout_counts(requeued: u64, dead_lettered: u64) -> Self {
        Self {
            acknowledge_outcome: AcknowledgeMessageOutcome::Acknowledged,
            timeout_requeued: requeued,
            timeout_dead_lettered: dead_lettered,
            timeout_batch_size: AtomicU32::new(0),
            expired_never_delivered: 0,
            expired_previously_delivered: 0,
            expiration_batch_size: AtomicU32::new(0),
        }
    }

    fn with_expired_counts(never_delivered: u64, previously_delivered: u64) -> Self {
        Self {
            acknowledge_outcome: AcknowledgeMessageOutcome::Acknowledged,
            timeout_requeued: 0,
            timeout_dead_lettered: 0,
            timeout_batch_size: AtomicU32::new(0),
            expired_never_delivered: never_delivered,
            expired_previously_delivered: previously_delivered,
            expiration_batch_size: AtomicU32::new(0),
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
        _queue_name: &str,
        _message_id: Uuid,
        _receipt_handle: Uuid,
    ) -> Result<AcknowledgeMessageOutcome, anyhow::Error> {
        Ok(match self.acknowledge_outcome {
            AcknowledgeMessageOutcome::Acknowledged => AcknowledgeMessageOutcome::Acknowledged,
            AcknowledgeMessageOutcome::QueueNotFound => AcknowledgeMessageOutcome::QueueNotFound,
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

    async fn process_expired_messages(
        &self,
        batch_size: u32,
    ) -> Result<ExpiredMessagesCleanupSummary, anyhow::Error> {
        self.expiration_batch_size
            .store(batch_size, Ordering::Relaxed);

        Ok(ExpiredMessagesCleanupSummary::new(vec![
            QueueExpiredMessagesCleanupSummary::new(
                "email-delivery".to_owned(),
                self.expired_never_delivered,
                self.expired_previously_delivered,
            ),
        ]))
    }
}

#[tokio::test]
async fn preserves_distinct_acknowledgement_failures() {
    async fn result_for(outcome: AcknowledgeMessageOutcome) -> Result<(), AcknowledgeMessageError> {
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

#[tokio::test]
async fn forwards_expiration_processing_and_preserves_the_summary() {
    let repository = FakeMessageRepository::with_expired_counts(31, 11);

    let summary = execute_process_expired_messages(&repository, 500)
        .await
        .expect("expired messages should be processed");

    assert_eq!(
        repository.expiration_batch_size.load(Ordering::Relaxed),
        500
    );
    assert_eq!(summary.processed(), 42);
    assert_eq!(summary.never_delivered(), 31);
    assert_eq!(summary.previously_delivered(), 11);

    let queue_summary = summary
        .per_queue()
        .first()
        .expect("summary should contain the affected queue");

    assert_eq!(queue_summary.queue_name(), "email-delivery");
    assert_eq!(queue_summary.never_delivered(), 31);
    assert_eq!(queue_summary.previously_delivered(), 11);
}
