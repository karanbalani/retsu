use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use uuid::Uuid;

use super::{CachedQueueRepository, CreateQueueOutcome, Queue, QueueDetails, QueueRepository};
use crate::{
    cache::{MemoryCache, MemoryCachePolicy},
    observability::test_metrics,
};

#[derive(Clone)]
struct FakeQueueRepository {
    state: Arc<FakeQueueRepositoryState>,
}

struct FakeQueueRepositoryState {
    create_outcome: CreateQueueOutcome,
    details: Mutex<Option<QueueDetails>>,
    create_calls: AtomicUsize,
    detail_calls: AtomicUsize,
}

impl FakeQueueRepository {
    fn new(create_outcome: CreateQueueOutcome, details: Option<QueueDetails>) -> Self {
        Self {
            state: Arc::new(FakeQueueRepositoryState {
                create_outcome,
                details: Mutex::new(details),
                create_calls: AtomicUsize::new(0),
                detail_calls: AtomicUsize::new(0),
            }),
        }
    }
}

impl QueueRepository for FakeQueueRepository {
    async fn create_queue(&self, queue: &Queue) -> Result<CreateQueueOutcome, anyhow::Error> {
        self.state.create_calls.fetch_add(1, Ordering::Relaxed);

        Ok(match self.state.create_outcome {
            CreateQueueOutcome::Created => {
                self.state
                    .details
                    .lock()
                    .expect("queue details lock should not be poisoned")
                    .replace(queue.details());

                CreateQueueOutcome::Created
            }
            CreateQueueOutcome::AlreadyExists => CreateQueueOutcome::AlreadyExists,
        })
    }

    async fn queue_details(&self, _queue_id: Uuid) -> Result<Option<QueueDetails>, anyhow::Error> {
        self.state.detail_calls.fetch_add(1, Ordering::Relaxed);

        Ok(self
            .state
            .details
            .lock()
            .expect("queue details lock should not be poisoned")
            .clone())
    }
}

fn cached(
    repository: FakeQueueRepository,
) -> CachedQueueRepository<FakeQueueRepository, MemoryCache<Uuid, QueueDetails>> {
    let (_, metrics) = test_metrics();
    let policy = MemoryCachePolicy::new(100, 1024 * 1024, Duration::from_secs(60));
    let cache = MemoryCache::new(
        "queue_details",
        policy,
        |queue_id: &Uuid, details: &QueueDetails| {
            u32::try_from(
                std::mem::size_of_val(queue_id)
                    + std::mem::size_of::<QueueDetails>()
                    + details.name().len(),
            )
            .unwrap_or(u32::MAX)
        },
        metrics.cache().clone(),
    );

    CachedQueueRepository::new(repository, cache)
}

#[tokio::test]
async fn reads_through_and_reuses_queue_details() {
    let queue_id = Uuid::now_v7();
    let details = QueueDetails::new(queue_id, "email-delivery".to_owned(), 45, 7, 300);
    let repository = FakeQueueRepository::new(CreateQueueOutcome::Created, Some(details.clone()));
    let cached = cached(repository.clone());

    for _ in 0..2 {
        let actual = cached
            .queue_details(queue_id)
            .await
            .expect("queue details lookup should succeed")
            .expect("queue should exist");

        assert_eq!(actual, details);
    }

    assert_eq!(repository.state.detail_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn populates_the_cache_only_after_successful_creation() {
    let repository = FakeQueueRepository::new(CreateQueueOutcome::Created, None);
    let cached = cached(repository.clone());
    let queue = Queue::new("email-delivery".to_owned(), Some(45), Some(7), Some(300))
        .expect("queue should be valid");

    let outcome = cached
        .create_queue(&queue)
        .await
        .expect("queue creation should succeed");
    assert!(matches!(outcome, CreateQueueOutcome::Created));

    let details = cached
        .queue_details(queue.id())
        .await
        .expect("queue details lookup should succeed")
        .expect("created queue should be cached");

    assert_eq!(details.id(), queue.id());
    assert_eq!(details.name(), queue.name());
    assert_eq!(repository.state.create_calls.load(Ordering::Relaxed), 1);
    assert_eq!(repository.state.detail_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn invalidation_forces_the_next_read_through_to_the_repository() {
    let queue_id = Uuid::now_v7();
    let initial = QueueDetails::new(queue_id, "email-delivery".to_owned(), 45, 7, 300);
    let repository = FakeQueueRepository::new(CreateQueueOutcome::Created, Some(initial));
    let cached = cached(repository.clone());

    cached
        .queue_details(queue_id)
        .await
        .expect("initial queue details lookup should succeed")
        .expect("queue should exist");

    let updated = QueueDetails::new(queue_id, "email-delivery".to_owned(), 90, 10, 600);
    repository
        .state
        .details
        .lock()
        .expect("queue details lock should not be poisoned")
        .replace(updated.clone());

    cached.invalidate_queue_details(queue_id).await;

    let actual = cached
        .queue_details(queue_id)
        .await
        .expect("queue details lookup after invalidation should succeed")
        .expect("queue should still exist");

    assert_eq!(actual, updated);
    assert_eq!(repository.state.detail_calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn does_not_cache_the_unpersisted_queue_after_a_conflict() {
    let persisted_id = Uuid::now_v7();
    let persisted = QueueDetails::new(persisted_id, "email-delivery".to_owned(), 30, 5, 604_800);
    let repository = FakeQueueRepository::new(CreateQueueOutcome::AlreadyExists, Some(persisted));
    let cached = cached(repository.clone());
    let conflicting = Queue::new("email-delivery".to_owned(), Some(60), Some(10), Some(300))
        .expect("queue should be valid");

    let outcome = cached
        .create_queue(&conflicting)
        .await
        .expect("conflict should be returned as an outcome");
    assert!(matches!(outcome, CreateQueueOutcome::AlreadyExists));

    let missing = cached
        .queue_details(conflicting.id())
        .await
        .expect("queue details lookup should succeed")
        .expect("fake repository returns its persisted queue");

    assert_eq!(missing.id(), persisted_id);
    assert_eq!(repository.state.detail_calls.load(Ordering::Relaxed), 1);
}
