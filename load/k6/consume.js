import execution from "k6/execution";

import {
    acknowledgeMessage,
    dequeueMessage,
    prepareQueues,
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
        consumers: {
            executor: "shared-iterations",
            vus: config.consumeVus,
            iterations: config.prefillMessages,
            maxDuration: config.consumeMaxDuration,
            gracefulStop: config.gracefulStop,
        },
    },
    thresholds: thresholdsFor(["dequeue", "acknowledge"], true),
};

export function setup() {
    return prepareQueues(config.prefillMessages);
}

export default function (data) {
    const iteration = execution.scenario.iterationInTest;
    const queue = data.queues[iteration % data.queues.length];
    const delivery = dequeueMessage(queue);

    if (delivery.failed) {
        recordLifecycle(false, "dequeue_failed");
        return;
    }

    if (delivery.empty) {
        return;
    }

    lifecycleStarted.add(1);
    recordLifecycle(
        acknowledgeMessage(queue, delivery.message),
        "acknowledge_failed",
    );
}
