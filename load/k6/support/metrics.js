import { Counter, Rate, Trend } from "k6/metrics";

export const operationDuration = new Trend("queue_operation_duration", true);
export const statusErrors = new Counter("status_errors");
export const statusErrorRate = new Rate("status_error_rate");
export const invalidResponses = new Counter("invalid_responses");
export const queuesCreated = new Counter("queues_created");
export const messagesEnqueued = new Counter("messages_enqueued");
export const messagesDequeued = new Counter("messages_dequeued");
export const messagesAcknowledged = new Counter("messages_acknowledged");
export const emptyDequeues = new Counter("empty_dequeues");
export const lifecycleStarted = new Counter("lifecycles_started");
export const lifecycleCompleted = new Counter("lifecycles_completed");
export const lifecycleErrors = new Counter("lifecycle_errors");
export const lifecycleCorrectnessRate = new Rate("lifecycle_correctness_rate");

export function recordResponse(response, operation, expectedStatuses) {
    const tags = { operation };
    const expected = expectedStatuses.includes(response.status);

    operationDuration.add(response.timings.duration, tags);
    statusErrorRate.add(!expected, tags);

    if (!expected) {
        statusErrors.add(1, {
            operation,
            status: String(response.status),
        });
    }

    return expected;
}

export function recordInvalidResponse(operation) {
    invalidResponses.add(1, { operation });
}

export function recordLifecycle(success, reason) {
    lifecycleCorrectnessRate.add(success);

    if (success) {
        lifecycleCompleted.add(1);
        return;
    }

    lifecycleErrors.add(1, { reason });
}
