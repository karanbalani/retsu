import { sleep } from "k6";
import execution from "k6/execution";

import {
    acknowledgeMessage,
    dequeueMessage,
    enqueueMessage,
    payloadFor,
    prepareQueues,
    priorityForIndex,
} from "./support/api.js";
import { config } from "./support/config.js";
import {
    lifecycleStarted,
    recordLifecycle,
} from "./support/metrics.js";
import { thresholdsFor } from "./support/options.js";

export const options = {
    setupTimeout: config.setupTimeout,
    scenarios: {
        producers: {
            executor: "constant-arrival-rate",
            exec: "produce",
            rate: config.mixedProducerRate,
            timeUnit: "1s",
            duration: config.mixedDuration,
            preAllocatedVUs: config.mixedPreAllocatedVus,
            maxVUs: config.mixedMaxVus,
            gracefulStop: config.gracefulStop,
        },
        consumers: {
            executor: "constant-arrival-rate",
            exec: "consume",
            startTime: "1s",
            rate: config.mixedConsumerRate,
            timeUnit: "1s",
            duration: config.mixedDuration,
            preAllocatedVUs: config.mixedPreAllocatedVus,
            maxVUs: config.mixedMaxVus,
            gracefulStop: config.gracefulStop,
        },
    },
    thresholds: thresholdsFor(["enqueue", "dequeue", "acknowledge"], true),
};

export function setup() {
    return prepareQueues(config.mixedPrefillMessages);
}

export function produce(data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const payload = payloadFor(`mixed-${data.runId}-${iteration}-`);

    const enqueued = enqueueMessage(
        queue,
        payload,
        priorityForIndex(iteration),
    );
    if (enqueued !== null) {
        void enqueued.id;
    }
}

export function consume(data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const delivery = dequeueMessage(queue);

    if (delivery.failed) {
        recordLifecycle(false, "dequeue_failed");
        return;
    }

    if (delivery.empty) {
        if (config.emptyDequeueSleepMs > 0) {
            sleep(config.emptyDequeueSleepMs / 1000);
        }
        return;
    }

    lifecycleStarted.add(1);
    recordLifecycle(
        acknowledgeMessage(queue, delivery.message),
        "acknowledge_failed",
    );
}
