function stringValue(name, fallback) {
    const value = __ENV[name];
    return value === undefined || value === "" ? fallback : value;
}

function integerValue(name, fallback, minimum, maximum) {
    const raw = stringValue(name, String(fallback));
    const value = Number(raw);

    if (!Number.isInteger(value) || value < minimum || value > maximum) {
        throw new Error(
            `${name} must be an integer between ${minimum} and ${maximum}; received ${raw}`,
        );
    }

    return value;
}

function numberValue(name, fallback, minimum, maximum) {
    const raw = stringValue(name, String(fallback));
    const value = Number(raw);

    if (!Number.isFinite(value) || value < minimum || value > maximum) {
        throw new Error(
            `${name} must be a number between ${minimum} and ${maximum}; received ${raw}`,
        );
    }

    return value;
}

function durationValue(name, fallback) {
    const value = stringValue(name, fallback);

    if (!/^\d+(?:\.\d+)?(?:ms|s|m|h)$/.test(value)) {
        throw new Error(
            `${name} must be a k6 duration such as 500ms, 30s, 2m, or 1h; received ${value}`,
        );
    }

    return value;
}

function httpUrl(name, value) {
    const normalized = value.replace(/\/+$/, "");

    if (!/^https?:\/\/[^/?#]+$/.test(normalized)) {
        throw new Error(
            `${name} must be an HTTP or HTTPS URL; received ${value}`,
        );
    }

    return normalized;
}

function priorityMix(value) {
    const weights = value.split(",").map((part) => Number(part.trim()));

    if (
        weights.length !== 3 ||
        weights.some((weight) => !Number.isInteger(weight) || weight < 0) ||
        weights.every((weight) => weight === 0)
    ) {
        throw new Error(
            `PRIORITY_MIX must contain three non-negative integer weights; received ${value}`,
        );
    }

    return Object.freeze({
        high: weights[0],
        medium: weights[1],
        low: weights[2],
        total: weights[0] + weights[1] + weights[2],
    });
}

export const config = Object.freeze({
    baseUrl: httpUrl(
        "BASE_URL",
        stringValue("BASE_URL", "http://127.0.0.1:2424"),
    ),
    prometheusUrl: httpUrl(
        "PROMETHEUS_URL",
        stringValue(
            "PROMETHEUS_URL",
            "http://127.0.0.1:24245",
        ),
    ),
    queueCount: integerValue("QUEUE_COUNT", 1, 1, 100),
    queuePrefix: stringValue("QUEUE_PREFIX", "retsu-k6"),
    runId: stringValue("RUN_ID", ""),
    payloadBytes: integerValue("PAYLOAD_BYTES", 1024, 1, 16 * 1024 * 1024),
    priorityMix: priorityMix(stringValue("PRIORITY_MIX", "20,60,20")),
    visibilityTimeoutSeconds: integerValue(
        "VISIBILITY_TIMEOUT_SECONDS",
        30,
        1,
        21_600,
    ),
    maxDeliveryAttempts: integerValue("MAX_DELIVERY_ATTEMPTS", 5, 1, 100),
    messageTtlSeconds: integerValue(
        "MESSAGE_TTL_SECONDS",
        3600,
        1,
        2_592_000,
    ),
    requestTimeout: durationValue("REQUEST_TIMEOUT", "5s"),
    setupTimeout: durationValue("SETUP_TIMEOUT", "5m"),
    gracefulStop: durationValue("GRACEFUL_STOP", "10s"),
    emptyDequeueSleepMs: integerValue("EMPTY_DEQUEUE_SLEEP_MS", 100, 0, 60_000),

    enqueueRate: integerValue("ENQUEUE_RATE", 10, 1, 100_000),
    enqueueDuration: durationValue("ENQUEUE_DURATION", "30s"),
    enqueuePreAllocatedVus: integerValue("ENQUEUE_PRE_ALLOCATED_VUS", 2, 1, 10_000),
    enqueueMaxVus: integerValue("ENQUEUE_MAX_VUS", 20, 1, 100_000),

    prefillMessages: integerValue("PREFILL_MESSAGES", 100, 1, 1_000_000),
    consumeVus: integerValue("CONSUME_VUS", 2, 1, 10_000),
    consumeMaxDuration: durationValue("CONSUME_MAX_DURATION", "5m"),

    mixedDuration: durationValue("MIXED_DURATION", "30s"),
    mixedPrefillMessages: integerValue(
        "MIXED_PREFILL_MESSAGES",
        20,
        0,
        1_000_000,
    ),
    mixedProducerRate: integerValue("MIXED_PRODUCER_RATE", 10, 1, 100_000),
    mixedConsumerRate: integerValue("MIXED_CONSUMER_RATE", 10, 1, 100_000),
    mixedPreAllocatedVus: integerValue("MIXED_PRE_ALLOCATED_VUS", 4, 1, 10_000),
    mixedMaxVus: integerValue("MIXED_MAX_VUS", 40, 1, 100_000),

    saturationPrefillMessages: integerValue(
        "SATURATION_PREFILL_MESSAGES",
        20,
        0,
        1_000_000,
    ),
    saturationStartRate: integerValue("SATURATION_START_RATE", 2, 1, 100_000),
    saturationRampRate: integerValue("SATURATION_RAMP_RATE", 10, 1, 100_000),
    saturationSpikeRate: integerValue("SATURATION_SPIKE_RATE", 20, 1, 100_000),
    saturationConsumerRatio: numberValue(
        "SATURATION_CONSUMER_RATIO",
        1,
        0.01,
        100,
    ),
    saturationWarmupDuration: durationValue("SATURATION_WARMUP_DURATION", "15s"),
    saturationRampDuration: durationValue("SATURATION_RAMP_DURATION", "30s"),
    saturationHoldDuration: durationValue("SATURATION_HOLD_DURATION", "30s"),
    saturationSpikeDuration: durationValue("SATURATION_SPIKE_DURATION", "10s"),
    saturationRecoveryDuration: durationValue(
        "SATURATION_RECOVERY_DURATION",
        "20s",
    ),
    saturationRampDownDuration: durationValue(
        "SATURATION_RAMP_DOWN_DURATION",
        "10s",
    ),
    saturationPreAllocatedVus: integerValue(
        "SATURATION_PRE_ALLOCATED_VUS",
        4,
        1,
        10_000,
    ),
    saturationMaxVus: integerValue("SATURATION_MAX_VUS", 50, 1, 100_000),

    productionDayHourSeconds: integerValue(
        "PRODUCTION_DAY_HOUR_SECONDS",
        60,
        2,
        3_600,
    ),
    productionDayTransitionSeconds: integerValue(
        "PRODUCTION_DAY_TRANSITION_SECONDS",
        1,
        1,
        300,
    ),
    productionDayConsumerHeadroom: numberValue(
        "PRODUCTION_DAY_CONSUMER_HEADROOM",
        1.3,
        1,
        10,
    ),
    productionDayProducerPreAllocatedVus: integerValue(
        "PRODUCTION_DAY_PRODUCER_PRE_ALLOCATED_VUS",
        256,
        1,
        10_000,
    ),
    productionDayProducerMaxVus: integerValue(
        "PRODUCTION_DAY_PRODUCER_MAX_VUS",
        512,
        1,
        100_000,
    ),
    productionDayConsumerPreAllocatedVus: integerValue(
        "PRODUCTION_DAY_CONSUMER_PRE_ALLOCATED_VUS",
        768,
        1,
        10_000,
    ),
    productionDayConsumerMaxVus: integerValue(
        "PRODUCTION_DAY_CONSUMER_MAX_VUS",
        1_024,
        1,
        100_000,
    ),
    productionDayDrainRate: integerValue(
        "PRODUCTION_DAY_DRAIN_RATE",
        325,
        1,
        100_000,
    ),
    productionDayDrainRampUpSeconds: integerValue(
        "PRODUCTION_DAY_DRAIN_RAMP_UP_SECONDS",
        1,
        1,
        300,
    ),
    productionDayDrainHoldSeconds: integerValue(
        "PRODUCTION_DAY_DRAIN_HOLD_SECONDS",
        29,
        1,
        3_600,
    ),
    productionDayDrainRampDownSeconds: integerValue(
        "PRODUCTION_DAY_DRAIN_RAMP_DOWN_SECONDS",
        2,
        1,
        300,
    ),
    productionDayCleanerWaitSeconds: integerValue(
        "PRODUCTION_DAY_CLEANER_WAIT_SECONDS",
        75,
        1,
        3_600,
    ),

    maximumStatusErrorRate: numberValue(
        "MAX_STATUS_ERROR_RATE",
        0.01,
        0,
        1,
    ),
    minimumCheckRate: numberValue("MIN_CHECK_RATE", 0.99, 0, 1),
    minimumLifecycleCorrectnessRate: numberValue(
        "MIN_LIFECYCLE_CORRECTNESS_RATE",
        0.99,
        0,
        1,
    ),
    enqueueP95Ms: integerValue("ENQUEUE_P95_MS", 750, 1, 600_000),
    dequeueP95Ms: integerValue("DEQUEUE_P95_MS", 750, 1, 600_000),
    acknowledgeP95Ms: integerValue("ACKNOWLEDGE_P95_MS", 750, 1, 600_000),
});

if (config.enqueueMaxVus < config.enqueuePreAllocatedVus) {
    throw new Error("ENQUEUE_MAX_VUS must be at least ENQUEUE_PRE_ALLOCATED_VUS");
}

if (config.mixedMaxVus < config.mixedPreAllocatedVus) {
    throw new Error("MIXED_MAX_VUS must be at least MIXED_PRE_ALLOCATED_VUS");
}

if (config.saturationMaxVus < config.saturationPreAllocatedVus) {
    throw new Error(
        "SATURATION_MAX_VUS must be at least SATURATION_PRE_ALLOCATED_VUS",
    );
}

if (
    config.productionDayTransitionSeconds >=
    config.productionDayHourSeconds
) {
    throw new Error(
        "PRODUCTION_DAY_TRANSITION_SECONDS must be less than PRODUCTION_DAY_HOUR_SECONDS",
    );
}

if (
    config.productionDayProducerMaxVus <
    config.productionDayProducerPreAllocatedVus
) {
    throw new Error(
        "PRODUCTION_DAY_PRODUCER_MAX_VUS must be at least PRODUCTION_DAY_PRODUCER_PRE_ALLOCATED_VUS",
    );
}

if (
    config.productionDayConsumerMaxVus <
    config.productionDayConsumerPreAllocatedVus
) {
    throw new Error(
        "PRODUCTION_DAY_CONSUMER_MAX_VUS must be at least PRODUCTION_DAY_CONSUMER_PRE_ALLOCATED_VUS",
    );
}
