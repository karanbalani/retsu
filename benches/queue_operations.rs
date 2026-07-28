use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use futures_util::future::join_all;

#[allow(dead_code)]
#[path = "../tests/integration/harness.rs"]
mod harness;

use harness::{IntegrationSystem, unique_queue_name};

const PAYLOAD_SIZE_BYTES: usize = 1024;
const LARGE_PAYLOAD_SIZE_BYTES: usize = 64 * 1024;
const DEQUEUE_QUEUE_DEPTHS: [u32; 3] = [1, 1_000, 10_000];

fn benchmark_queue_operations(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should start");

    let system = runtime
        .block_on(IntegrationSystem::start())
        .expect("benchmark system should start");
    let payload = "x".repeat(PAYLOAD_SIZE_BYTES);
    let large_payload = "x".repeat(LARGE_PAYLOAD_SIZE_BYTES);

    let enqueue_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-enqueue");
    let acknowledge_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-acknowledge");
    let lifecycle_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-lifecycle");
    let large_lifecycle_queue_id =
        create_benchmark_queue(&runtime, &system, "benchmark-large-lifecycle");
    let concurrent_4_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-concurrent-4");
    let concurrent_8_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-concurrent-8");
    let dequeue_queues = DEQUEUE_QUEUE_DEPTHS.map(|depth| {
        let queue_id = create_benchmark_queue(
            &runtime,
            &system,
            &format!("benchmark-dequeue-depth-{depth}"),
        );

        runtime
            .block_on(system.seed_ready_messages_directly(
                queue_id,
                depth,
                PAYLOAD_SIZE_BYTES as u32,
            ))
            .expect("dequeue benchmark messages should be seeded");

        (depth, queue_id)
    });

    let mut group = criterion.benchmark_group("queue_operations");

    group.bench_function("enqueue/1_kib", |bencher| {
        bencher.to_async(&runtime).iter_custom(|iterations| {
            measure_enqueue_iterations(&system, enqueue_queue_id, payload.as_str(), iterations)
        });
    });

    for (depth, queue_id) in dequeue_queues {
        group.bench_function(
            format!("dequeue/1_kib_depth_{}", format_depth(depth)),
            |bencher| {
                bencher.to_async(&runtime).iter_custom(|iterations| {
                    measure_dequeue_iterations(&system, queue_id, iterations)
                });
            },
        );
    }

    group.bench_function("acknowledge/1_kib", |bencher| {
        bencher.to_async(&runtime).iter_custom(|iterations| {
            measure_acknowledge_iterations(
                &system,
                acknowledge_queue_id,
                payload.as_str(),
                iterations,
            )
        });
    });

    group.bench_function("lifecycle/1_kib", |bencher| {
        bencher
            .to_async(&runtime)
            .iter(|| execute_lifecycle(&system, lifecycle_queue_id, payload.as_str()));
    });

    group.bench_function("lifecycle/64_kib", |bencher| {
        bencher
            .to_async(&runtime)
            .iter(|| execute_lifecycle(&system, large_lifecycle_queue_id, large_payload.as_str()));
    });

    group.finish();

    let mut concurrency_group = criterion.benchmark_group("concurrent_lifecycle");

    concurrency_group.throughput(Throughput::Elements(4));
    concurrency_group.bench_function("4_workers/1_kib", |bencher| {
        bencher.to_async(&runtime).iter(|| {
            execute_concurrent_lifecycles(&system, concurrent_4_queue_id, payload.as_str(), 4)
        });
    });

    concurrency_group.throughput(Throughput::Elements(8));
    concurrency_group.bench_function("8_workers/1_kib", |bencher| {
        bencher.to_async(&runtime).iter(|| {
            execute_concurrent_lifecycles(&system, concurrent_8_queue_id, payload.as_str(), 8)
        });
    });

    concurrency_group.finish();

    runtime.block_on(async {
        drop(system);
    });
}

fn create_benchmark_queue(
    runtime: &tokio::runtime::Runtime,
    system: &IntegrationSystem,
    name_prefix: &str,
) -> uuid::Uuid {
    let queue_name = unique_queue_name(name_prefix);

    runtime
        .block_on(system.create_queue(&queue_name, 30, 5, 604_800))
        .expect("benchmark queue should be created")
}

async fn measure_enqueue_iterations(
    system: &IntegrationSystem,
    queue_id: uuid::Uuid,
    payload: &str,
    iterations: u64,
) -> Duration {
    let mut measured = Duration::ZERO;

    for _ in 0..iterations {
        let started = Instant::now();
        let message_id = system
            .enqueue_message(queue_id, payload, "MEDIUM", None)
            .await
            .expect("benchmark message should be enqueued");
        measured += started.elapsed();

        system
            .delete_message_directly(message_id)
            .await
            .expect("benchmark message should be deleted");
    }

    measured
}

async fn measure_dequeue_iterations(
    system: &IntegrationSystem,
    queue_id: uuid::Uuid,
    iterations: u64,
) -> Duration {
    let mut measured = Duration::ZERO;

    for _ in 0..iterations {
        let started = Instant::now();
        let message = system
            .dequeue_message(queue_id)
            .await
            .expect("benchmark message should be dequeued")
            .expect("benchmark queue should contain seeded messages");
        measured += started.elapsed();

        system
            .restore_message_directly(message.id)
            .await
            .expect("benchmark message should be restored");
    }

    measured
}

async fn measure_acknowledge_iterations(
    system: &IntegrationSystem,
    queue_id: uuid::Uuid,
    payload: &str,
    iterations: u64,
) -> Duration {
    let mut measured = Duration::ZERO;

    for _ in 0..iterations {
        let message_id = system
            .enqueue_message(queue_id, payload, "MEDIUM", None)
            .await
            .expect("benchmark message should be enqueued");
        let message = system
            .dequeue_message(queue_id)
            .await
            .expect("benchmark message should be dequeued")
            .expect("benchmark queue should contain one message");

        assert_eq!(message.id, message_id);

        let started = Instant::now();
        system
            .acknowledge_message(queue_id, message.id, message.receipt_handle)
            .await
            .expect("benchmark message should be acknowledged");
        measured += started.elapsed();
    }

    measured
}

async fn execute_lifecycle(system: &IntegrationSystem, queue_id: uuid::Uuid, payload: &str) {
    let (enqueued_message_id, dequeued_message_id) =
        execute_worker_lifecycle(system, queue_id, payload).await;

    assert_eq!(dequeued_message_id, enqueued_message_id);
}

async fn execute_worker_lifecycle(
    system: &IntegrationSystem,
    queue_id: uuid::Uuid,
    payload: &str,
) -> (uuid::Uuid, uuid::Uuid) {
    let enqueued_message_id = system
        .enqueue_message(queue_id, payload, "MEDIUM", None)
        .await
        .expect("benchmark message should be enqueued");
    let message = system
        .dequeue_message(queue_id)
        .await
        .expect("benchmark message should be dequeued")
        .expect("benchmark queue should contain one message");

    system
        .acknowledge_message(queue_id, message.id, message.receipt_handle)
        .await
        .expect("benchmark message should be acknowledged");

    (enqueued_message_id, message.id)
}

async fn execute_concurrent_lifecycles(
    system: &IntegrationSystem,
    queue_id: uuid::Uuid,
    payload: &str,
    concurrency: u8,
) {
    assert!(concurrency > 0);

    let enqueued_message_ids = join_all(
        (0..concurrency).map(|_| system.enqueue_message(queue_id, payload, "MEDIUM", None)),
    )
    .await
    .into_iter()
    .map(|result| result.expect("concurrent benchmark message should be enqueued"))
    .collect::<HashSet<_>>();

    let dequeued_messages = join_all((0..concurrency).map(|_| system.dequeue_message(queue_id)))
        .await
        .into_iter()
        .map(|result| {
            result
                .expect("concurrent benchmark message should be dequeued")
                .expect("concurrent benchmark queue should contain every enqueued message")
        })
        .collect::<Vec<_>>();
    let dequeued_message_ids = dequeued_messages
        .iter()
        .map(|message| message.id)
        .collect::<HashSet<_>>();

    assert_eq!(dequeued_message_ids, enqueued_message_ids);

    join_all(
        dequeued_messages.into_iter().map(|message| {
            system.acknowledge_message(queue_id, message.id, message.receipt_handle)
        }),
    )
    .await
    .into_iter()
    .for_each(|result| result.expect("concurrent benchmark message should be acknowledged"));
}

fn format_depth(depth: u32) -> String {
    match depth {
        1_000 => "1k".to_owned(),
        10_000 => "10k".to_owned(),
        _ => depth.to_string(),
    }
}

criterion_group!(benches, benchmark_queue_operations);
criterion_main!(benches);
