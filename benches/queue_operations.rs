use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};

#[allow(dead_code)]
#[path = "../tests/integration/harness.rs"]
mod harness;

use harness::{IntegrationSystem, unique_queue_name};

const PAYLOAD_SIZE_BYTES: usize = 1024;
const DEQUEUE_QUEUE_DEPTH: u32 = 1_000;

fn benchmark_queue_operations(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should start");

    let system = runtime
        .block_on(IntegrationSystem::start())
        .expect("benchmark system should start");
    let payload = "x".repeat(PAYLOAD_SIZE_BYTES);

    let enqueue_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-enqueue");
    let dequeue_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-dequeue");
    let acknowledge_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-acknowledge");
    let lifecycle_queue_id = create_benchmark_queue(&runtime, &system, "benchmark-lifecycle");

    runtime
        .block_on(system.seed_ready_messages_directly(
            dequeue_queue_id,
            DEQUEUE_QUEUE_DEPTH,
            PAYLOAD_SIZE_BYTES as u32,
        ))
        .expect("dequeue benchmark messages should be seeded");

    let mut group = criterion.benchmark_group("queue_operations");

    group.bench_function("enqueue/1_kib", |bencher| {
        bencher.to_async(&runtime).iter_custom(|iterations| {
            measure_enqueue_iterations(&system, enqueue_queue_id, payload.as_str(), iterations)
        });
    });

    group.bench_function("dequeue/1_kib_depth_1k", |bencher| {
        bencher.to_async(&runtime).iter_custom(|iterations| {
            measure_dequeue_iterations(&system, dequeue_queue_id, iterations)
        });
    });

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
        bencher.to_async(&runtime).iter(|| async {
            let message_id = system
                .enqueue_message(lifecycle_queue_id, &payload, "MEDIUM", None)
                .await
                .expect("benchmark message should be enqueued");
            let message = system
                .dequeue_message(lifecycle_queue_id)
                .await
                .expect("benchmark message should be dequeued")
                .expect("benchmark queue should contain one message");

            assert_eq!(message.id, message_id);

            system
                .acknowledge_message(lifecycle_queue_id, message.id, message.receipt_handle)
                .await
                .expect("benchmark message should be acknowledged");
        });
    });

    group.finish();

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

criterion_group!(benches, benchmark_queue_operations);
criterion_main!(benches);
