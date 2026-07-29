import { check } from "k6";
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
        smoke: {
            executor: "shared-iterations",
            vus: Math.min(config.queueCount, 10),
            iterations: config.queueCount,
            maxDuration: "2m",
        },
    },
    thresholds: thresholdsFor(["enqueue", "dequeue", "acknowledge"], true),
};

export function setup() {
    return prepareQueues();
}

export default function (data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const priority = priorityForIndex(iteration);
    const payload = payloadFor(`smoke-${data.runId}-${iteration}-`);

    lifecycleStarted.add(1);

    const enqueued = enqueueMessage(queue, payload, priority);
    if (enqueued === null) {
        recordLifecycle(false, "enqueue_failed");
        return;
    }

    const delivery = dequeueMessage(queue);
    if (delivery.failed || delivery.empty) {
        recordLifecycle(false, delivery.empty ? "unexpected_empty" : "dequeue_failed");
        return;
    }

    const messageMatches = check(
        delivery.message,
        {
            "dequeued message id matches enqueue": (message) =>
                message.id === enqueued.id,
            "dequeued payload matches enqueue": (message) =>
                message.payload === enqueued.payload,
            "dequeued priority matches enqueue": (message) =>
                message.priority === enqueued.priority,
            "first delivery attempt is recorded": (message) =>
                message.delivery_attempts === 1,
        },
        { operation: "lifecycle" },
    );

    const acknowledged = acknowledgeMessage(queue, delivery.message);
    if (!acknowledged) {
        recordLifecycle(false, "acknowledge_failed");
        return;
    }

    const afterAcknowledge = dequeueMessage(queue);
    const queueIsEmpty = check(
        afterAcknowledge,
        {
            "acknowledged message is no longer available": (result) =>
                result.empty === true,
        },
        { operation: "lifecycle" },
    );

    recordLifecycle(messageMatches && queueIsEmpty, "lifecycle_mismatch");
}
