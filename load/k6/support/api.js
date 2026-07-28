import { check } from "k6";
import http from "k6/http";

import { config } from "./config.js";
import {
    emptyDequeues,
    messagesAcknowledged,
    messagesDequeued,
    messagesEnqueued,
    queuesCreated,
    recordInvalidResponse,
    recordResponse,
} from "./metrics.js";

const UUID_PATTERN =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function requestParameters(operation, expectedStatuses) {
    return {
        headers: {
            "Content-Type": "application/json",
        },
        redirects: 0,
        responseCallback: http.expectedStatuses(...expectedStatuses),
        tags: {
            operation,
        },
        timeout: config.requestTimeout,
    };
}

function parseJson(response, operation) {
    try {
        return response.json();
    } catch (_error) {
        recordInvalidResponse(operation);
        return null;
    }
}

function normalizeQueueComponent(value, fallback) {
    const normalized = value
        .toLowerCase()
        .replace(/[^a-z0-9._-]+/g, "-")
        .replace(/^[^a-z0-9]+|[^a-z0-9]+$/g, "");

    return normalized || fallback;
}

function createRunId() {
    if (config.runId !== "") {
        return normalizeQueueComponent(config.runId, "run");
    }

    const timestamp = Date.now().toString(36);
    const random = Math.floor(Math.random() * 0x1000000)
        .toString(36)
        .padStart(5, "0");
    return `${timestamp}-${random}`;
}

function queueName(prefix, runId, index) {
    const normalizedRunId = normalizeQueueComponent(runId, "run")
        .slice(0, 24)
        .replace(/[^a-z0-9]+$/g, "");
    const suffix = `-${normalizedRunId || "run"}-${index + 1}`;
    const availablePrefixBytes = Math.max(1, 64 - suffix.length);
    const shortenedPrefix = normalizeQueueComponent(prefix, "retsu-k6")
        .slice(0, availablePrefixBytes)
        .replace(/[^a-z0-9]+$/g, "");

    return `${shortenedPrefix || "q"}${suffix}`;
}

export function isUuid(value) {
    return typeof value === "string" && UUID_PATTERN.test(value);
}

export function payloadFor(marker, bytes = config.payloadBytes) {
    if (marker.length >= bytes) {
        return marker.slice(0, bytes);
    }

    return marker + "x".repeat(bytes - marker.length);
}

export function priorityForFraction(fraction) {
    const weighted = fraction * config.priorityMix.total;

    if (weighted < config.priorityMix.high) {
        return "HIGH";
    }

    if (weighted < config.priorityMix.high + config.priorityMix.medium) {
        return "MEDIUM";
    }

    return "LOW";
}

export function priorityForIndex(index) {
    return priorityForFraction(
        (index % config.priorityMix.total) / config.priorityMix.total,
    );
}

export function createQueue(name) {
    const operation = "create_queue";
    const response = http.post(
        `${config.baseUrl}/v1/queues`,
        JSON.stringify({
            name,
            visibility_timeout_seconds: config.visibilityTimeoutSeconds,
            max_delivery_attempts: config.maxDeliveryAttempts,
            default_message_ttl_seconds: config.messageTtlSeconds,
        }),
        requestParameters(operation, [201]),
    );
    const expected = recordResponse(response, operation, [201]);
    const body = expected ? parseJson(response, operation) : null;
    const valid = expected && isUuid(body && body.id);

    check(
        response,
        {
            "queue creation returns 201": () => response.status === 201,
            "queue creation returns an id": () => valid,
        },
        { operation },
    );

    if (!valid) {
        throw new Error(
            `queue creation failed for ${name}: status ${response.status}, body ${response.body}`,
        );
    }

    queuesCreated.add(1);
    return {
        id: body.id,
        name,
    };
}

export function enqueueMessage(
    queue,
    payload,
    priority,
    operation = "enqueue",
) {
    const response = http.post(
        `${config.baseUrl}/v1/queues/${queue.id}/messages`,
        JSON.stringify({
            payload,
            priority,
            ttl_seconds: config.messageTtlSeconds,
        }),
        requestParameters(operation, [201]),
    );
    const expected = recordResponse(response, operation, [201]);
    const body = expected ? parseJson(response, operation) : null;
    const valid = expected && isUuid(body && body.id);

    check(
        response,
        {
            "enqueue returns 201": () => response.status === 201,
            "enqueue returns a message id": () => valid,
        },
        { operation },
    );

    if (!valid) {
        return null;
    }

    messagesEnqueued.add(1, { operation });
    return {
        id: body.id,
        payload,
        priority,
    };
}

export function dequeueMessage(queue) {
    const operation = "dequeue";
    const response = http.post(
        `${config.baseUrl}/v1/queues/${queue.id}/messages/dequeue`,
        null,
        requestParameters(operation, [200, 204]),
    );
    const expected = recordResponse(response, operation, [200, 204]);

    check(
        response,
        {
            "dequeue returns 200 or 204": () =>
                response.status === 200 || response.status === 204,
        },
        { operation },
    );

    if (!expected) {
        return { failed: true };
    }

    if (response.status === 204) {
        emptyDequeues.add(1);
        return { empty: true };
    }

    const body = parseJson(response, operation);
    const valid =
        body !== null &&
        isUuid(body.id) &&
        typeof body.payload === "string" &&
        ["HIGH", "MEDIUM", "LOW"].includes(body.priority) &&
        isUuid(body.receipt_handle) &&
        Number.isInteger(body.delivery_attempts);

    check(
        body,
        {
            "dequeue returns a valid message": () => valid,
        },
        { operation },
    );

    if (!valid) {
        return { failed: true };
    }

    messagesDequeued.add(1);
    return {
        message: body,
    };
}

export function acknowledgeMessage(queue, message) {
    const operation = "acknowledge";
    const response = http.post(
        `${config.baseUrl}/v1/queues/${queue.id}/messages/${message.id}/acknowledge`,
        JSON.stringify({
            receipt_handle: message.receipt_handle,
        }),
        requestParameters(operation, [204]),
    );
    const expected = recordResponse(response, operation, [204]);

    check(
        response,
        {
            "acknowledge returns 204": () => response.status === 204,
        },
        { operation },
    );

    if (expected) {
        messagesAcknowledged.add(1);
    }

    return expected;
}

export function prepareQueues(prefillMessages = 0) {
    const runId = createRunId();
    const queues = [];

    for (let index = 0; index < config.queueCount; index += 1) {
        queues.push(createQueue(queueName(config.queuePrefix, runId, index)));
    }

    for (let index = 0; index < prefillMessages; index += 1) {
        const queue = queues[index % queues.length];
        const marker = `prefill-${runId}-${index}-`;
        const enqueued = enqueueMessage(
            queue,
            payloadFor(marker),
            priorityForIndex(index),
            "prefill_enqueue",
        );

        if (enqueued === null) {
            throw new Error(
                `prefill failed at message ${index + 1} of ${prefillMessages}`,
            );
        }
    }

    return {
        prefillMessages,
        queues,
        runId,
    };
}
