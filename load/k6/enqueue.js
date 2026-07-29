import execution from "k6/execution";

import {
    enqueueMessage,
    payloadFor,
    prepareQueues,
    priorityForIndex,
} from "./support/api.js";
import { config } from "./support/config.js";
import { thresholdsFor } from "./support/options.js";

export const options = {
    setupTimeout: config.setupTimeout,
    scenarios: {
        producers: {
            executor: "constant-arrival-rate",
            rate: config.enqueueRate,
            timeUnit: "1s",
            duration: config.enqueueDuration,
            preAllocatedVUs: config.enqueuePreAllocatedVus,
            maxVUs: config.enqueueMaxVus,
            gracefulStop: config.gracefulStop,
        },
    },
    thresholds: thresholdsFor(["enqueue"]),
};

export function setup() {
    return prepareQueues();
}

export default function (data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const payload = payloadFor(`enqueue-${data.runId}-${iteration}-`);
    const priority = priorityForIndex(iteration);

    const enqueued = enqueueMessage(queue, payload, priority);
    if (enqueued !== null) {
        void enqueued.id;
    }
}
