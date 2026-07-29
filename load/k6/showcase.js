import { check, sleep } from "k6";
import execution from "k6/execution";
import http from "k6/http";
import { Counter, Trend } from "k6/metrics";

import {
    acknowledgeMessage,
    dequeueMessage,
    enqueueMessage,
    prepareQueueProfiles,
} from "./support/api.js";
import { config } from "./support/config.js";
import {
    lifecycleStarted,
    recordLifecycle,
} from "./support/metrics.js";
import { thresholdsFor } from "./support/options.js";
import {
    SHOWCASE_QUEUE_PROFILES,
    SHOWCASE_START_RATE,
    SHOWCASE_VERIFICATION_WINDOW_SECONDS,
    expectedShowcaseCohorts,
    expectedShowcaseDeliveryAccounting,
    expectedShowcaseEnqueues,
    expectedShowcaseMainConsumerIterations,
    expectedShowcasePriorities,
    expectedShowcaseQueues,
    expectedShowcaseTailIterations,
    showcaseActiveLoadMinutes,
    showcaseCohort,
    showcaseConsumerRate,
    showcaseConsumerStages,
    showcasePriority,
    showcaseProducerStages,
    showcaseQueueIndex,
    showcaseVerificationStartSeconds,
} from "./support/showcase.js";

const NORMAL_TTL_SECONDS = 180;
const FAULT_TTL_SECONDS = 120;
const EXPIRY_TTL_SECONDS = 6;
const plannedEnqueues = new Counter("showcase_planned_enqueues");
const successfulEnqueues = new Counter(
    "showcase_successful_enqueues",
);
const consumerIterations = new Counter(
    "showcase_consumer_iterations",
);
const plannedFaults = new Counter("showcase_planned_faults");
const queueSelections = new Counter("showcase_queue_selections");
const deliveryAttempts = new Counter(
    "showcase_delivery_attempts",
);
const intentionalNoAcks = new Counter(
    "showcase_intentional_no_acks",
);
const terminalOutcomes = new Counter(
    "showcase_terminal_outcomes",
);
const unexpectedOutcomes = new Counter(
    "showcase_unexpected_outcomes",
);
const serverOutcomes = new Counter("showcase_server_outcomes");
const serverVerificationFailures = new Counter(
    "showcase_server_verification_failures",
);
const processingDuration = new Trend(
    "showcase_processing_duration",
    true,
);

const showcaseSettings = Object.freeze({
    durationMinutes: config.showcaseDurationMinutes,
    activeLoadMinutes: showcaseActiveLoadMinutes(
        config.showcaseDurationMinutes,
    ),
    headroom: config.showcaseConsumerHeadroom,
    drainRate: config.showcaseDrainRate,
    drainRampUpSeconds: config.showcaseDrainRampUpSeconds,
    drainHoldSeconds: config.showcaseDrainHoldSeconds,
    drainRampDownSeconds: config.showcaseDrainRampDownSeconds,
});

const consumerStartRate = showcaseConsumerRate(
    SHOWCASE_START_RATE,
    showcaseSettings.headroom,
);
const expectedEnqueues = expectedShowcaseEnqueues(
    showcaseSettings.activeLoadMinutes,
);
const expectedMainConsumerIterations =
    expectedShowcaseMainConsumerIterations(
        showcaseSettings.activeLoadMinutes,
        showcaseSettings.headroom,
    );
const expectedTailConsumerIterations =
    expectedShowcaseTailIterations(showcaseSettings);
const expectedConsumerIterations =
    expectedMainConsumerIterations + expectedTailConsumerIterations;
const expectedCohorts = expectedShowcaseCohorts(expectedEnqueues);
const expectedPriorities =
    expectedShowcasePriorities(expectedEnqueues);
const expectedQueues = expectedShowcaseQueues(expectedEnqueues);
const expectedDeliveryAccounting =
    expectedShowcaseDeliveryAccounting(expectedEnqueues);
const verificationStartSeconds =
    showcaseVerificationStartSeconds(showcaseSettings);

const thresholds = {
    ...thresholdsFor(["enqueue", "dequeue", "acknowledge"], true),
    dropped_iterations: ["count==0"],
    showcase_consumer_iterations: [
        `count==${expectedConsumerIterations}`,
    ],
    showcase_planned_enqueues: [`count==${expectedEnqueues}`],
    showcase_planned_faults: [
        `count==${expectedDeliveryAccounting.faultMessages}`,
    ],
    showcase_server_verification_failures: ["count==0"],
    showcase_successful_enqueues: [`count==${expectedEnqueues}`],
    showcase_unexpected_outcomes: ["count==0"],
};

export const options = {
    setupTimeout: config.setupTimeout,
    scenarios: {
        producers: {
            executor: "ramping-arrival-rate",
            exec: "produce",
            startRate: SHOWCASE_START_RATE,
            timeUnit: "1s",
            stages: showcaseProducerStages(
                showcaseSettings.activeLoadMinutes,
            ),
            preAllocatedVUs:
                config.showcaseProducerPreAllocatedVus,
            maxVUs: config.showcaseProducerMaxVus,
            gracefulStop: config.gracefulStop,
        },
        consumers: {
            executor: "ramping-arrival-rate",
            exec: "consume",
            startRate: consumerStartRate,
            timeUnit: "1s",
            stages: showcaseConsumerStages(showcaseSettings),
            preAllocatedVUs:
                config.showcaseConsumerPreAllocatedVus,
            maxVUs: config.showcaseConsumerMaxVus,
            gracefulStop: config.gracefulStop,
        },
        verify_server_outcomes: {
            executor: "per-vu-iterations",
            exec: "verifyServerOutcomes",
            vus: 1,
            iterations: 1,
            startTime: `${verificationStartSeconds}s`,
            maxDuration:
                `${SHOWCASE_VERIFICATION_WINDOW_SECONDS + 1}s`,
            gracefulStop: "0s",
        },
    },
    tags: {
        test_type: "showcase",
    },
    thresholds,
};

export function setup() {
    console.log(
        `showcase plan: ${expectedEnqueues} enqueues, ` +
            `${showcaseSettings.activeLoadMinutes} active-load minutes, ` +
            `${expectedMainConsumerIterations} showcase consumer iterations, ` +
            `${expectedTailConsumerIterations} drain iterations`,
    );
    console.log(
        "showcase expected distribution: " +
            `priorities HIGH/MEDIUM/LOW=` +
            `${expectedPriorities.HIGH}/${expectedPriorities.MEDIUM}/` +
            `${expectedPriorities.LOW}; queues hot-a/hot-b/warm-a/` +
            `warm-b/fault=${expectedQueues["hot-a"]}/` +
            `${expectedQueues["hot-b"]}/${expectedQueues["warm-a"]}/` +
            `${expectedQueues["warm-b"]}/${expectedQueues.fault}`,
    );
    console.log(
        "showcase expected faults: " +
            `retry-once=${expectedCohorts.retry_once}, ` +
            `retry-twice=${expectedCohorts.retry_twice}, ` +
            `dead-letter=${expectedCohorts.dead_letter}, ` +
            `expiry=${expectedCohorts.expiry}`,
    );
    console.log(
        "showcase expected outcomes: " +
            `attempts 1/2/3=${expectedDeliveryAccounting.attempts.first}/` +
            `${expectedDeliveryAccounting.attempts.second}/` +
            `${expectedDeliveryAccounting.attempts.third}; ` +
            `no-acks=${expectedDeliveryAccounting.intentionalNoAcks}; ` +
            `acks=${expectedDeliveryAccounting.acknowledgements.total}; ` +
            `DLQ=${expectedDeliveryAccounting.deadLetters}; ` +
            `previously-delivered expiry=` +
            `${expectedDeliveryAccounting.previouslyDeliveredExpirations}`,
    );

    return prepareQueueProfiles(SHOWCASE_QUEUE_PROFILES);
}

function queueTags(queue, phase, cohort = "none", faultKind = "none") {
    return {
        phase,
        queue: queue.profile,
        queue_class: queue.queueClass,
        cohort,
        fault_kind: faultKind,
    };
}

function payloadForShowcase(data, iteration, cohort) {
    return JSON.stringify({
        event_id: `${data.runId}-${iteration}`,
        tenant_id: `tenant-${iteration % 100}`,
        event_type: "work.created",
        source: "showcase",
        cohort:
            cohort.faultKind === "none"
                ? cohort.cohort
                : `fault:${cohort.faultKind}`,
    });
}

function ttlFor(cohort) {
    if (cohort.faultKind === "expiry") {
        return EXPIRY_TTL_SECONDS;
    }
    if (cohort.faultKind !== "none") {
        return FAULT_TTL_SECONDS;
    }
    return NORMAL_TTL_SECONDS;
}

function parsePayload(payload) {
    try {
        const parsed = JSON.parse(payload);
        if (
            parsed === null ||
            typeof parsed !== "object" ||
            Array.isArray(parsed) ||
            Object.keys(parsed).length !== 5 ||
            typeof parsed.event_id !== "string" ||
            typeof parsed.tenant_id !== "string" ||
            parsed.event_type !== "work.created" ||
            parsed.source !== "showcase" ||
            typeof parsed.cohort !== "string"
        ) {
            return null;
        }

        if (parsed.cohort.startsWith("fault:")) {
            const faultKind = parsed.cohort.slice("fault:".length);
            if (
                ![
                    "retry_once",
                    "retry_twice",
                    "dead_letter",
                    "expiry",
                ].includes(faultKind)
            ) {
                return null;
            }
            return {
                cohort: "fault",
                faultKind,
                processingSeconds: faultKind === "expiry" ? 5 : 1,
            };
        }

        const processingSecondsByCohort = {
            process_1s: 1,
            process_2s: 2,
            process_3s: 3,
            process_5s: 5,
        };
        const processingSeconds =
            processingSecondsByCohort[parsed.cohort];

        if (processingSeconds === undefined) {
            return null;
        }

        return {
            cohort: parsed.cohort,
            faultKind: "none",
            processingSeconds,
        };
    } catch (_error) {
        return null;
    }
}

function attemptTag(attempt) {
    if (attempt <= 1) {
        return "1";
    }
    if (attempt === 2) {
        return "2";
    }
    return "3";
}

function expectedAction(cohort, attempt) {
    switch (cohort.faultKind) {
        case "none":
            return attempt === 1 ? "ack" : "unexpected_ack";
        case "retry_once":
            if (attempt === 1) {
                return "skip";
            }
            return attempt === 2 ? "ack" : "unexpected_ack";
        case "retry_twice":
            if (attempt <= 2) {
                return "skip";
            }
            return attempt === 3 ? "ack" : "unexpected_ack";
        case "dead_letter":
            return attempt <= 3 ? "skip" : "unexpected_skip";
        case "expiry":
            return attempt === 1 ? "skip" : "unexpected_skip";
        default:
            return "unexpected_ack";
    }
}

export function produce(data) {
    const iteration = execution.scenario.iterationInTest;
    const queue =
        data.queues[showcaseQueueIndex(iteration, "producer")];
    const cohort = showcaseCohort(iteration);
    const priority = showcasePriority(iteration);
    const tags = queueTags(
        queue,
        "active",
        cohort.cohort,
        cohort.faultKind,
    );
    tags.message_priority = priority;

    plannedEnqueues.add(1, tags);
    queueSelections.add(1, { role: "producer", ...tags });
    if (cohort.faultKind !== "none") {
        plannedFaults.add(1, {
            fault_kind: cohort.faultKind,
            queue: queue.profile,
        });
    }

    const enqueued = enqueueMessage(
        queue,
        payloadForShowcase(data, iteration, cohort),
        priority,
        "enqueue",
        {
            tags,
            ttlSeconds: ttlFor(cohort),
        },
    );
    if (enqueued !== null) {
        successfulEnqueues.add(1, tags);
    }
}

export function consume(data) {
    const iteration = execution.scenario.iterationInTest;
    const phase =
        iteration < expectedMainConsumerIterations ? "active" : "drain";
    const queue =
        data.queues[showcaseQueueIndex(iteration, "consumer")];
    const baseTags = queueTags(queue, phase);

    consumerIterations.add(1, { phase });
    queueSelections.add(1, { role: "consumer", ...baseTags });

    const delivery = dequeueMessage(queue, { tags: baseTags });
    if (delivery.failed) {
        recordLifecycle(false, "dequeue_failed");
        return;
    }
    if (delivery.empty) {
        return;
    }

    const cohort = parsePayload(delivery.message.payload);
    const validPayload = check(
        cohort,
        {
            "showcase payload has five valid fields": (value) =>
                value !== null,
        },
        { operation: "lifecycle", phase },
    );
    if (!validPayload) {
        recordLifecycle(false, "invalid_showcase_payload");
        acknowledgeMessage(queue, delivery.message, {
            tags: {
                ...baseTags,
                cohort: "invalid",
                fault_kind: "none",
                delivery_attempt: attemptTag(
                    delivery.message.delivery_attempts,
                ),
            },
        });
        return;
    }

    const attempt = delivery.message.delivery_attempts;
    const action = expectedAction(cohort, attempt);
    const tags = {
        ...baseTags,
        cohort: cohort.cohort,
        fault_kind: cohort.faultKind,
        delivery_attempt: attemptTag(attempt),
        message_priority: delivery.message.priority,
    };

    if (attempt === 1) {
        lifecycleStarted.add(1);
    }
    unexpectedOutcomes.add(action.startsWith("unexpected") ? 1 : 0);
    deliveryAttempts.add(1, tags);
    processingDuration.add(cohort.processingSeconds * 1000, tags);
    sleep(cohort.processingSeconds);

    if (action === "skip") {
        intentionalNoAcks.add(1, {
            ...tags,
            outcome: "intentional_no_ack",
        });
        return;
    }

    if (action.startsWith("unexpected")) {
        recordLifecycle(false, "unexpected_delivery_attempt");
    }

    const acknowledged = acknowledgeMessage(queue, delivery.message, {
        tags,
    });
    if (!acknowledged) {
        recordLifecycle(false, "acknowledge_failed");
        return;
    }

    terminalOutcomes.add(1, {
        ...tags,
        outcome: "acknowledged",
    });
    if (action === "ack") {
        recordLifecycle(true, "acknowledged");
    }
}

function prometheusQueueMatcher(queues) {
    const escapedNames = queues.map((queue) =>
        queue.name.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&"),
    );
    return `^(${escapedNames.join("|")})$`;
}

function prometheusScalar(query, outcome) {
    const response = http.get(
        `${config.prometheusUrl}/api/v1/query?query=${encodeURIComponent(query)}`,
        {
            redirects: 0,
            responseCallback: http.expectedStatuses(200),
            tags: {
                operation: "verify_server_outcomes",
                outcome,
            },
            timeout: config.requestTimeout,
        },
    );

    let value = null;
    if (response.status === 200) {
        try {
            const body = response.json();
            const rawValue =
                body &&
                body.status === "success" &&
                body.data &&
                Array.isArray(body.data.result) &&
                body.data.result.length === 1 &&
                Array.isArray(body.data.result[0].value)
                    ? body.data.result[0].value[1]
                    : null;
            const parsed = Number(rawValue);
            if (Number.isInteger(parsed) && parsed >= 0) {
                value = parsed;
            }
        } catch (_error) {
            value = null;
        }
    }

    const valid = value !== null;
    check(
        response,
        {
            [`Prometheus returned ${outcome}`]: () => valid,
        },
        { operation: "verify_server_outcomes", outcome },
    );
    return value;
}

export function verifyServerOutcomes(data) {
    const verificationStartedAt = Date.now();
    const queueMatcher = prometheusQueueMatcher(data.queues);
    const outcomes = [
        {
            outcome: "dead_lettered",
            expected: expectedDeliveryAccounting.deadLetters,
            query:
                `sum(queue_messages_dead_lettered_total{` +
                `queue_name=~"${queueMatcher}"}) or vector(0)`,
        },
        {
            outcome: "previously_delivered_expired",
            expected:
                expectedDeliveryAccounting.previouslyDeliveredExpirations,
            query:
                `sum(queue_messages_expired_total{` +
                `queue_name=~"${queueMatcher}",` +
                `message_delivery_history="previously_delivered"}) ` +
                `or vector(0)`,
        },
        {
            outcome: "never_delivered_expired",
            expected: 0,
            query:
                `sum(queue_messages_expired_total{` +
                `queue_name=~"${queueMatcher}",` +
                `message_delivery_history="never_delivered"}) ` +
                `or vector(0)`,
        },
        {
            outcome: "state_collection_success",
            expected: 1,
            query:
                `max(queue_state_collection_success{` +
                `component="state-metrics-collector"}) or vector(0)`,
        },
        {
            outcome: "state_snapshot_fresh",
            expected: 1,
            query:
                `(max(queue_state_snapshot_age_seconds{` +
                `component="state-metrics-collector"}) <= bool 30) ` +
                `or vector(0)`,
        },
        {
            outcome: "queue_state_series",
            expected: SHOWCASE_QUEUE_PROFILES.length * 3,
            query:
                `count(queue_messages_ready{` +
                `queue_name=~"${queueMatcher}"})`,
        },
        {
            outcome: "ready_messages",
            expected: 0,
            query:
                `sum(queue_messages_ready{` +
                `queue_name=~"${queueMatcher}"}) or vector(0)`,
        },
        {
            outcome: "in_flight_messages",
            expected: 0,
            query:
                `sum(queue_messages_in_flight{` +
                `queue_name=~"${queueMatcher}"}) or vector(0)`,
        },
    ];

    for (const outcome of outcomes) {
        const observed = prometheusScalar(
            outcome.query,
            outcome.outcome,
        );
        const matchesExpected = observed === outcome.expected;
        check(
            observed,
            {
                [`${outcome.outcome} matched planned state`]:
                    () => matchesExpected,
            },
            {
                operation: "verify_server_outcomes",
                outcome: outcome.outcome,
            },
        );
        serverVerificationFailures.add(
            matchesExpected ? 0 : 1,
            { outcome: outcome.outcome },
        );
        serverOutcomes.add(observed ?? 0, {
            outcome: outcome.outcome,
        });
        console.log(
            `showcase server outcome ${outcome.outcome}: ` +
                `observed=${observed ?? "unavailable"}, ` +
                `expected=${outcome.expected}`,
        );
    }

    const elapsedSeconds =
        (Date.now() - verificationStartedAt) / 1000;
    sleep(
        Math.max(
            0,
            SHOWCASE_VERIFICATION_WINDOW_SECONDS - elapsedSeconds,
        ),
    );
}
