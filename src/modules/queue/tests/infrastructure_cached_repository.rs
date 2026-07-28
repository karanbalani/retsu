use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use uuid::Uuid;

use super::{CreateQueueOutcome, Queue, QueueNameCachingRepository, QueueRepository};
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
    name: Mutex<Option<String>>,
    create_calls: AtomicUsize,
    name_calls: AtomicUsize,
}

impl FakeQueueRepository {
    fn new(create_outcome: CreateQueueOutcome, name: Option<String>) -> Self {
        Self {
            state: Arc::new(FakeQueueRepositoryState {
                create_outcome,
                name: Mutex::new(name),
                create_calls: AtomicUsize::new(0),
                name_calls: AtomicUsize::new(0),
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
                    .name
                    .lock()
                    .expect("queue name lock should not be poisoned")
                    .replace(queue.name().to_owned());

                CreateQueueOutcome::Created
            }
            CreateQueueOutcome::AlreadyExists => CreateQueueOutcome::AlreadyExists,
        })
    }

    async fn queue_name(&self, _queue_id: Uuid) -> Result<Option<String>, anyhow::Error> {
        self.state.name_calls.fetch_add(1, Ordering::Relaxed);

        Ok(self
            .state
            .name
            .lock()
            .expect("queue name lock should not be poisoned")
            .clone())
    }
}

fn cached(
    repository: FakeQueueRepository,
) -> QueueNameCachingRepository<FakeQueueRepository, MemoryCache<Uuid, String>> {
    let (_, metrics) = test_metrics();
    let cache = MemoryCache::new(
        "queue_names",
        MemoryCachePolicy::new(100, 1024 * 1024),
        |queue_id: &Uuid, queue_name: &String| {
            u32::try_from(
                std::mem::size_of_val(queue_id)
                    + std::mem::size_of::<String>()
                    + queue_name.capacity(),
            )
            .unwrap_or(u32::MAX)
        },
        metrics.cache().clone(),
    );

    QueueNameCachingRepository::new(repository, cache)
}

#[tokio::test]
async fn reads_through_and_reuses_queue_names() {
    let queue_id = Uuid::now_v7();
    let repository = FakeQueueRepository::new(
        CreateQueueOutcome::Created,
        Some("email-delivery".to_owned()),
    );
    let cached = cached(repository.clone());

    for _ in 0..2 {
        let actual = cached
            .queue_name(queue_id)
            .await
            .expect("queue name lookup should succeed")
            .expect("queue should exist");

        assert_eq!(actual, "email-delivery");
    }

    assert_eq!(repository.state.name_calls.load(Ordering::Relaxed), 1);
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

    let name = cached
        .queue_name(queue.id())
        .await
        .expect("queue name lookup should succeed")
        .expect("created queue name should be cached");

    assert_eq!(name, queue.name());
    assert_eq!(repository.state.create_calls.load(Ordering::Relaxed), 1);
    assert_eq!(repository.state.name_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn invalidation_forces_the_next_read_through_to_the_repository() {
    let queue_id = Uuid::now_v7();
    let repository = FakeQueueRepository::new(
        CreateQueueOutcome::Created,
        Some("email-delivery".to_owned()),
    );
    let cached = cached(repository.clone());

    cached
        .queue_name(queue_id)
        .await
        .expect("initial queue name lookup should succeed")
        .expect("queue should exist");

    repository
        .state
        .name
        .lock()
        .expect("queue name lock should not be poisoned")
        .replace("email-delivery-v2".to_owned());

    cached.invalidate_queue_name(queue_id).await;

    let actual = cached
        .queue_name(queue_id)
        .await
        .expect("queue name lookup after invalidation should succeed")
        .expect("queue should still exist");

    assert_eq!(actual, "email-delivery-v2");
    assert_eq!(repository.state.name_calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn does_not_cache_the_unpersisted_queue_name_after_a_conflict() {
    let repository = FakeQueueRepository::new(
        CreateQueueOutcome::AlreadyExists,
        Some("email-delivery".to_owned()),
    );
    let cached = cached(repository.clone());
    let conflicting = Queue::new("email-delivery".to_owned(), Some(60), Some(10), Some(300))
        .expect("queue should be valid");

    let outcome = cached
        .create_queue(&conflicting)
        .await
        .expect("conflict should be returned as an outcome");
    assert!(matches!(outcome, CreateQueueOutcome::AlreadyExists));

    let name = cached
        .queue_name(conflicting.id())
        .await
        .expect("queue name lookup should succeed")
        .expect("fake repository returns the persisted queue name");

    assert_eq!(name, "email-delivery");
    assert_eq!(repository.state.name_calls.load(Ordering::Relaxed), 1);
}
