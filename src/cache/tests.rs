use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::anyhow;

use super::{Cache, MemoryCache, MemoryCachePolicy};
use crate::observability::test_metrics;

fn cache() -> MemoryCache<u64, String> {
    let (_, metrics) = test_metrics();
    let policy = MemoryCachePolicy::new(100, 1024 * 1024);

    MemoryCache::new(
        "test_values",
        policy,
        |key: &u64, value: &String| {
            u32::try_from(
                std::mem::size_of_val(key) + std::mem::size_of::<String>() + value.capacity(),
            )
            .unwrap_or(u32::MAX)
        },
        metrics.cache().clone(),
    )
}

#[tokio::test]
async fn loads_once_and_reuses_the_cached_value() {
    let cache = cache();
    let loads = AtomicUsize::new(0);

    for _ in 0..2 {
        let value = cache
            .get_or_load(42, || async {
                loads.fetch_add(1, Ordering::Relaxed);
                Ok::<_, anyhow::Error>(Some("value".to_owned()))
            })
            .await
            .expect("loader should succeed")
            .expect("loader should return a value");

        assert_eq!(value.as_str(), "value");
    }

    assert_eq!(loads.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn coalesces_concurrent_loads_for_the_same_key() {
    let cache = cache();
    let loads = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();

    for _ in 0..8 {
        let cache = cache.clone();
        let loads = Arc::clone(&loads);

        tasks.push(tokio::spawn(async move {
            cache
                .get_or_load(42, || async move {
                    loads.fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok::<_, anyhow::Error>(Some("value".to_owned()))
                })
                .await
                .expect("loader should succeed")
                .expect("loader should return a value")
        }));
    }

    for task in tasks {
        let value = task.await.expect("cache task should complete");
        assert_eq!(value.as_str(), "value");
    }

    assert_eq!(loads.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn does_not_cache_missing_values_or_loader_errors() {
    let cache = cache();
    let missing_loads = AtomicUsize::new(0);

    for _ in 0..2 {
        let value = cache
            .get_or_load(42, || async {
                missing_loads.fetch_add(1, Ordering::Relaxed);
                Ok::<_, anyhow::Error>(None)
            })
            .await
            .expect("missing lookup should not fail");

        assert!(value.is_none());
    }

    let error_loads = AtomicUsize::new(0);
    for _ in 0..2 {
        let error = cache
            .get_or_load(43, || async {
                error_loads.fetch_add(1, Ordering::Relaxed);
                Err::<Option<String>, _>(anyhow!("load failed"))
            })
            .await
            .expect_err("loader error should be returned");

        assert_eq!(error.to_string(), "load failed");
    }

    assert_eq!(missing_loads.load(Ordering::Relaxed), 2);
    assert_eq!(error_loads.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn explicit_insert_replaces_the_cached_value() {
    let cache = cache();

    cache
        .insert(42, Arc::new("first".to_owned()))
        .await
        .expect("initial insert should succeed");
    cache
        .insert(42, Arc::new("second".to_owned()))
        .await
        .expect("replacement insert should succeed");

    let value = cache
        .get_or_load(42, || async {
            unreachable!("explicitly inserted value should be cached");
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(None)
        })
        .await
        .expect("cache lookup should succeed")
        .expect("cache should contain the inserted value");

    assert_eq!(value.as_str(), "second");
}

#[tokio::test]
async fn enforces_entry_and_weighted_byte_limits() {
    let (_, metrics) = test_metrics();
    let entry_limited = MemoryCache::new(
        "entry_limited",
        MemoryCachePolicy::new(2, 1_000),
        |_key: &u64, _value: &String| 1,
        metrics.cache().clone(),
    );

    for key in 0..3 {
        entry_limited
            .insert(key, Arc::new(format!("value-{key}")))
            .await
            .expect("insert should succeed");
    }
    entry_limited.run_pending_tasks().await;

    assert!(entry_limited.entry_count() <= 2);
    assert!(entry_limited.weighted_size() <= 1_000);

    let byte_limited = MemoryCache::new(
        "byte_limited",
        MemoryCachePolicy::new(100, 100),
        |_key: &u64, value: &String| u32::try_from(value.len()).unwrap_or(u32::MAX),
        metrics.cache().clone(),
    );

    for key in 0..3 {
        byte_limited
            .insert(key, Arc::new("x".repeat(40)))
            .await
            .expect("insert should succeed");
    }
    byte_limited.run_pending_tasks().await;

    assert!(byte_limited.entry_count() <= 2);
    assert!(byte_limited.weighted_size() <= 100);
}
