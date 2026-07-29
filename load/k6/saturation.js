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
import { saturationStages, thresholdsFor } from "./support/options.js";

export const options = {
    setupTimeout: config.setupTimeout,
    scenarios: {
        producers: {
            executor: "ramping-arrival-rate",
            exec: "produce",
            startRate: config.saturationStartRate,
            timeUnit: "1s",
            stages: saturationStages(),
            preAllocatedVUs: config.saturationPreAllocatedVus,
            maxVUs: config.saturationMaxVus,
            gracefulStop: config.gracefulStop,
        },
        consumers: {
            executor: "ramping-arrival-rate",
            exec: "consume",
            startTime: "1s",
            startRate: Math.max(
                1,
                Math.round(
                    config.saturationStartRate *
                        config.saturationConsumerRatio,
                ),
            ),
            timeUnit: "1s",
            stages: saturationStages(config.saturationConsumerRatio),
            preAllocatedVUs: config.saturationPreAllocatedVus,
            maxVUs: config.saturationMaxVus,
            gracefulStop: config.gracefulStop,
        },
    },
    thresholds: thresholdsFor(["enqueue", "dequeue", "acknowledge"], true),
};

export function setup() {
    return prepareQueues(config.saturationPrefillMessages);
}

export function produce(data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const payload = payloadFor(`saturation-${data.runId}-${iteration}-`);

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
