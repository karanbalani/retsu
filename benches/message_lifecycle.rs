use criterion::{Criterion, criterion_group, criterion_main};

#[allow(dead_code)]
#[path = "../tests/integration/harness.rs"]
mod harness;

use harness::{IntegrationSystem, unique_queue_name};

const PAYLOAD_SIZE_BYTES: usize = 1024;

fn benchmark_message_lifecycle(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("benchmark runtime should start");

    let system = runtime
        .block_on(IntegrationSystem::start())
        .expect("benchmark system should start");
    let queue_name = unique_queue_name("benchmark-lifecycle");
    let payload = "x".repeat(PAYLOAD_SIZE_BYTES);

    let queue_id = runtime
        .block_on(system.create_queue(&queue_name, 30, 5, 604_800))
        .expect("benchmark queue should be created");

    criterion.bench_function("message_lifecycle/1_kib", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            let message_id = system
                .enqueue_message(queue_id, &payload, "MEDIUM", None)
                .await
                .expect("benchmark message should be enqueued");
            let message = system
                .dequeue_message(queue_id)
                .await
                .expect("benchmark message should be dequeued")
                .expect("benchmark queue should contain one message");

            assert_eq!(message.id, message_id);

            system
                .acknowledge_message(queue_id, message.id, message.receipt_handle)
                .await
                .expect("benchmark message should be acknowledged");
        });
    });

    runtime.block_on(async {
        drop(system);
    });
}

criterion_group!(benches, benchmark_message_lifecycle);
criterion_main!(benches);
