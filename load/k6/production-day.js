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
    PRODUCTION_DAY_HOURLY_RATES,
    PRODUCTION_DAY_QUEUE_PROFILES,
    expectedProductionDayCohorts,
    expectedProductionDayDeliveryAccounting,
    expectedProductionDayEnqueues,
    expectedProductionDayMainConsumerIterations,
    expectedProductionDayPriorities,
    expectedProductionDayQueues,
    expectedProductionDayTailIterations,
    productionDayCohort,
    productionDayConsumerRates,
    productionDayConsumerStages,
    productionDayPriority,
    productionDayProducerStages,
    productionDayQueueIndex,
    productionDayVerificationStartSeconds,
} from "./support/production-day.js";

const NORMAL_TTL_SECONDS = 1_800;
const FAULT_TTL_SECONDS = 120;
const EXPIRY_TTL_SECONDS = 6;
const MAX_UNEXPECTED_SERVICE_ERROR_RATE = 0.0001;
const MINIMUM_CORRECTNESS_RATE = 0.9999;

const plannedEnqueues = new Counter("production_day_planned_enqueues");
const successfulEnqueues = new Counter(
    "production_day_successful_enqueues",
);
const consumerIterations = new Counter(
    "production_day_consumer_iterations",
);
const plannedFaults = new Counter("production_day_planned_faults");
const queueSelections = new Counter("production_day_queue_selections");
const deliveryAttempts = new Counter(
    "production_day_delivery_attempts",
);
const intentionalNoAcks = new Counter(
    "production_day_intentional_no_acks",
);
const terminalOutcomes = new Counter(
    "production_day_terminal_outcomes",
);
const unexpectedOutcomes = new Counter(
    "production_day_unexpected_outcomes",
);
const serverOutcomes = new Counter("production_day_server_outcomes");
const serverVerificationFailures = new Counter(
    "production_day_server_verification_failures",
);
const processingDuration = new Trend(
    "production_day_processing_duration",
    true,
);

const daySettings = Object.freeze({
    hourSeconds: config.productionDayHourSeconds,
    transitionSeconds: config.productionDayTransitionSeconds,
    headroom: config.productionDayConsumerHeadroom,
    drainRate: config.productionDayDrainRate,
    drainRampUpSeconds: config.productionDayDrainRampUpSeconds,
    drainHoldSeconds: config.productionDayDrainHoldSeconds,
    drainRampDownSeconds: config.productionDayDrainRampDownSeconds,
    cleanerWaitSeconds: config.productionDayCleanerWaitSeconds,
});

const consumerRates = productionDayConsumerRates(daySettings.headroom);
const expectedEnqueues = expectedProductionDayEnqueues(
    daySettings.hourSeconds,
);
const expectedMainConsumerIterations =
    expectedProductionDayMainConsumerIterations(
        daySettings.hourSeconds,
        daySettings.headroom,
    );
const expectedTailConsumerIterations =
    expectedProductionDayTailIterations(daySettings);
const expectedConsumerIterations =
    expectedMainConsumerIterations + expectedTailConsumerIterations;
const expectedCohorts = expectedProductionDayCohorts(expectedEnqueues);
const expectedPriorities =
    expectedProductionDayPriorities(expectedEnqueues);
const expectedQueues = expectedProductionDayQueues(expectedEnqueues);
const expectedDeliveryAccounting =
    expectedProductionDayDeliveryAccounting(expectedEnqueues);
const verificationStartSeconds =
    productionDayVerificationStartSeconds(daySettings);

const thresholds = {
    ...thresholdsFor(["enqueue", "dequeue", "acknowledge"], true),
    checks: [`rate>=${MINIMUM_CORRECTNESS_RATE}`],
    http_req_failed: [
        `rate<=${MAX_UNEXPECTED_SERVICE_ERROR_RATE}`,
    ],
    status_error_rate: [
        `rate<=${MAX_UNEXPECTED_SERVICE_ERROR_RATE}`,
    ],
    lifecycle_correctness_rate: [
        `rate>=${MINIMUM_CORRECTNESS_RATE}`,
    ],
    dropped_iterations: ["count==0"],
    production_day_planned_enqueues: [`count==${expectedEnqueues}`],
    production_day_successful_enqueues: [
        `count==${expectedEnqueues}`,
    ],
    production_day_consumer_iterations: [
        `count==${expectedConsumerIterations}`,
    ],
    production_day_delivery_attempts: [
        `count==${expectedDeliveryAccounting.attempts.total}`,
    ],
    production_day_intentional_no_acks: [
        `count==${expectedDeliveryAccounting.intentionalNoAcks}`,
    ],
    production_day_terminal_outcomes: [
        `count==${expectedDeliveryAccounting.acknowledgements.total}`,
    ],
    production_day_unexpected_outcomes: ["count==0"],
    production_day_server_verification_failures: ["count==0"],
    lifecycles_started: [
        `count==${expectedDeliveryAccounting.attempts.first}`,
    ],
    messages_acknowledged: [
        `count==${expectedDeliveryAccounting.acknowledgements.total}`,
    ],
};

for (const [priority, count] of Object.entries(expectedPriorities)) {
    thresholds[
        `production_day_successful_enqueues{message_priority:${priority}}`
    ] = [`count==${count}`];
}

for (const [queue, count] of Object.entries(expectedQueues)) {
    thresholds[`production_day_successful_enqueues{queue:${queue}}`] = [
        `count==${count}`,
    ];
}

for (const cohort of [
    "process_1s",
    "process_2s",
    "process_3s",
    "process_5s",
]) {
    thresholds[`production_day_successful_enqueues{cohort:${cohort}}`] = [
        `count==${expectedCohorts[cohort]}`,
    ];
}
thresholds["production_day_successful_enqueues{cohort:fault}"] = [
    `count==${expectedDeliveryAccounting.faultMessages}`,
];

for (const faultKind of [
    "retry_once",
    "retry_twice",
    "dead_letter",
    "expiry",
]) {
    thresholds[
        `production_day_planned_faults{fault_kind:${faultKind}}`
    ] = [`count==${expectedCohorts[faultKind]}`];
}

for (const [attempt, count] of [
    ["1", expectedDeliveryAccounting.attempts.first],
    ["2", expectedDeliveryAccounting.attempts.second],
    ["3", expectedDeliveryAccounting.attempts.third],
]) {
    thresholds[
        `production_day_delivery_attempts{delivery_attempt:${attempt}}`
    ] = [`count==${count}`];
}

for (const [faultKind, count] of [
    ["retry_once", expectedCohorts.retry_once],
    ["retry_twice", expectedCohorts.retry_twice * 2],
    ["dead_letter", expectedCohorts.dead_letter * 3],
    ["expiry", expectedCohorts.expiry],
]) {
    thresholds[
        `production_day_intentional_no_acks{fault_kind:${faultKind}}`
    ] = [`count==${count}`];
}

for (const [attempt, count] of [
    ["1", expectedDeliveryAccounting.acknowledgements.first],
    ["2", expectedDeliveryAccounting.acknowledgements.second],
    ["3", expectedDeliveryAccounting.acknowledgements.third],
]) {
    thresholds[
        `production_day_terminal_outcomes{delivery_attempt:${attempt}}`
    ] = [`count==${count}`];
}

for (const [faultKind, count] of [
    ["none", expectedDeliveryAccounting.acknowledgements.first],
    ["retry_once", expectedCohorts.retry_once],
    ["retry_twice", expectedCohorts.retry_twice],
]) {
    thresholds[
        `production_day_terminal_outcomes{fault_kind:${faultKind}}`
    ] = [`count==${count}`];
}

for (const [outcome, count] of [
    ["dead_lettered", expectedDeliveryAccounting.deadLetters],
    [
        "previously_delivered_expired",
        expectedDeliveryAccounting.previouslyDeliveredExpirations,
    ],
    ["never_delivered_expired", 0],
    ["state_collection_success", 1],
    ["state_snapshot_fresh", 1],
    ["queue_state_series", PRODUCTION_DAY_QUEUE_PROFILES.length * 3],
    ["ready_messages", 0],
    ["in_flight_messages", 0],
]) {
    thresholds[`production_day_server_outcomes{outcome:${outcome}}`] = [
        `count==${count}`,
    ];
}

export const options = {
    setupTimeout: config.setupTimeout,
    scenarios: {
        producers: {
            executor: "ramping-arrival-rate",
            exec: "produce",
            startRate: PRODUCTION_DAY_HOURLY_RATES[0],
            timeUnit: "1s",
            stages: productionDayProducerStages(
                daySettings.hourSeconds,
                daySettings.transitionSeconds,
            ),
            preAllocatedVUs:
                config.productionDayProducerPreAllocatedVus,
            maxVUs: config.productionDayProducerMaxVus,
            gracefulStop: config.gracefulStop,
        },
        consumers: {
            executor: "ramping-arrival-rate",
            exec: "consume",
            startRate: consumerRates[0],
            timeUnit: "1s",
            stages: productionDayConsumerStages(daySettings),
            preAllocatedVUs:
                config.productionDayConsumerPreAllocatedVus,
            maxVUs: config.productionDayConsumerMaxVus,
            gracefulStop: config.gracefulStop,
        },
        verify_server_outcomes: {
            executor: "per-vu-iterations",
            exec: "verifyServerOutcomes",
            vus: 1,
            iterations: 1,
            startTime: `${verificationStartSeconds}s`,
            maxDuration: "1m",
        },
    },
    thresholds,
};

export function setup() {
    console.log(
        `production-day plan: ${expectedEnqueues} enqueues, ` +
            `${expectedMainConsumerIterations} day consumer iterations, ` +
            `${expectedTailConsumerIterations} drain iterations`,
    );
    console.log(
        "production-day expected distribution: " +
            `priorities HIGH/MEDIUM/LOW=` +
            `${expectedPriorities.HIGH}/${expectedPriorities.MEDIUM}/` +
            `${expectedPriorities.LOW}; queues hot-a/hot-b/warm-a/` +
            `warm-b/fault=${expectedQueues["hot-a"]}/` +
            `${expectedQueues["hot-b"]}/${expectedQueues["warm-a"]}/` +
            `${expectedQueues["warm-b"]}/${expectedQueues.fault}`,
    );
    console.log(
        "production-day expected faults: " +
            `retry-once=${expectedCohorts.retry_once}, ` +
            `retry-twice=${expectedCohorts.retry_twice}, ` +
            `dead-letter=${expectedCohorts.dead_letter}, ` +
            `expiry=${expectedCohorts.expiry}`,
    );
    console.log(
        "production-day expected outcomes: " +
            `attempts 1/2/3=${expectedDeliveryAccounting.attempts.first}/` +
            `${expectedDeliveryAccounting.attempts.second}/` +
            `${expectedDeliveryAccounting.attempts.third}; ` +
            `no-acks=${expectedDeliveryAccounting.intentionalNoAcks}; ` +
            `acks=${expectedDeliveryAccounting.acknowledgements.total}; ` +
            `DLQ=${expectedDeliveryAccounting.deadLetters}; ` +
            `previously-delivered expiry=` +
            `${expectedDeliveryAccounting.previouslyDeliveredExpirations}`,
    );

    return prepareQueueProfiles(PRODUCTION_DAY_QUEUE_PROFILES);
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

function payloadForProductionDay(data, iteration, cohort) {
    return JSON.stringify({
        event_id: `${data.runId}-${iteration}`,
        tenant_id: `tenant-${iteration % 100}`,
        event_type: "work.created",
        source: "production-day",
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
            parsed.source !== "production-day" ||
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
        data.queues[productionDayQueueIndex(iteration, "producer")];
    const cohort = productionDayCohort(iteration);
    const priority = productionDayPriority(iteration);
    const tags = queueTags(
        queue,
        "day",
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
        payloadForProductionDay(data, iteration, cohort),
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
        iteration < expectedMainConsumerIterations ? "day" : "drain";
    const queue =
        data.queues[productionDayQueueIndex(iteration, "consumer")];
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
            "production-day payload has five valid fields": (value) =>
                value !== null,
        },
        { operation: "lifecycle", phase },
    );
    if (!validPayload) {
        recordLifecycle(false, "invalid_production_day_payload");
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
    serverVerificationFailures.add(valid ? 0 : 1, { outcome });
    return value;
}

export function verifyServerOutcomes(data) {
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
            expected: PRODUCTION_DAY_QUEUE_PROFILES.length * 3,
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
        serverOutcomes.add(observed ?? 0, {
            outcome: outcome.outcome,
        });
        console.log(
            `production-day server outcome ${outcome.outcome}: ` +
                `observed=${observed ?? "unavailable"}, ` +
                `expected=${outcome.expected}`,
        );
    }
}
