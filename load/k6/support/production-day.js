export const PRODUCTION_DAY_HOURLY_RATES = Object.freeze([
    50, 50, 50, 50, 50, 50, 50, 150, 250, 100, 90, 100,
    150, 250, 120, 100, 90, 50, 50, 50, 50, 150, 250, 50,
]);

export const PRODUCTION_DAY_QUEUE_PROFILES = Object.freeze([
    Object.freeze({
        profile: "hot-a",
        queueClass: "hot",
        weight: 35,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 8,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 1_800,
        }),
    }),
    Object.freeze({
        profile: "hot-b",
        queueClass: "hot",
        weight: 35,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 8,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 1_800,
        }),
    }),
    Object.freeze({
        profile: "warm-a",
        queueClass: "warm",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 8,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 1_800,
        }),
    }),
    Object.freeze({
        profile: "warm-b",
        queueClass: "warm",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 8,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 1_800,
        }),
    }),
    Object.freeze({
        profile: "fault",
        queueClass: "fault",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 8,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 1_800,
        }),
    }),
]);

const FAULT_PATTERN = Object.freeze([
    "retry_once",
    "retry_twice",
    "dead_letter",
    "expiry",
    "retry_once",
    "retry_twice",
    "retry_once",
    "retry_once",
    "retry_once",
]);

function duration(seconds) {
    return `${seconds}s`;
}

function weightedIndex(slot, denominator) {
    let upperBound = 0;

    for (
        let index = 0;
        index < PRODUCTION_DAY_QUEUE_PROFILES.length;
        index += 1
    ) {
        upperBound +=
            (PRODUCTION_DAY_QUEUE_PROFILES[index].weight * denominator) /
            100;
        if (slot < upperBound) {
            return index;
        }
    }

    return PRODUCTION_DAY_QUEUE_PROFILES.length - 1;
}

function trapezoidIterations(startRate, endRate, seconds) {
    const iterations = ((startRate + endRate) * seconds) / 2;

    if (!Number.isInteger(iterations)) {
        throw new Error(
            `rate transition from ${startRate} to ${endRate} over ${seconds}s does not produce an integer iteration count`,
        );
    }

    return iterations;
}

export function productionDayProducerStages(
    hourSeconds,
    transitionSeconds,
) {
    const holdSeconds = hourSeconds - transitionSeconds;
    const stages = [
        {
            duration: duration(hourSeconds),
            target: PRODUCTION_DAY_HOURLY_RATES[0],
        },
    ];

    for (
        let hour = 1;
        hour < PRODUCTION_DAY_HOURLY_RATES.length;
        hour += 1
    ) {
        stages.push({
            duration: duration(transitionSeconds),
            target: PRODUCTION_DAY_HOURLY_RATES[hour],
        });
        stages.push({
            duration: duration(holdSeconds),
            target: PRODUCTION_DAY_HOURLY_RATES[hour],
        });
    }

    return stages;
}

export function productionDayConsumerRates(headroom) {
    return PRODUCTION_DAY_HOURLY_RATES.map((rate) => {
        const target = rate * headroom;

        if (!Number.isInteger(target)) {
            throw new Error(
                `consumer headroom ${headroom} does not produce an integer target for ${rate}/s`,
            );
        }

        return target;
    });
}

export function productionDayConsumerStages(settings) {
    const rates = productionDayConsumerRates(settings.headroom);
    const holdSeconds = settings.hourSeconds - settings.transitionSeconds;
    const stages = [
        {
            duration: duration(settings.hourSeconds),
            target: rates[0],
        },
    ];

    for (let hour = 1; hour < rates.length; hour += 1) {
        stages.push({
            duration: duration(settings.transitionSeconds),
            target: rates[hour],
        });
        stages.push({
            duration: duration(holdSeconds),
            target: rates[hour],
        });
    }

    stages.push({
        duration: duration(settings.drainRampUpSeconds),
        target: settings.drainRate,
    });
    stages.push({
        duration: duration(settings.drainHoldSeconds),
        target: settings.drainRate,
    });
    stages.push({
        duration: duration(settings.drainRampDownSeconds),
        target: 0,
    });

    return stages;
}

export function productionDayVerificationStartSeconds(settings) {
    return (
        PRODUCTION_DAY_HOURLY_RATES.length * settings.hourSeconds +
        settings.drainRampUpSeconds +
        settings.drainHoldSeconds +
        settings.drainRampDownSeconds +
        settings.cleanerWaitSeconds
    );
}

export function expectedProductionDayEnqueues(hourSeconds) {
    return (
        PRODUCTION_DAY_HOURLY_RATES.reduce(
            (total, rate) => total + rate,
            0,
        ) * hourSeconds
    );
}

export function expectedProductionDayMainConsumerIterations(
    hourSeconds,
    headroom,
) {
    return (
        productionDayConsumerRates(headroom).reduce(
            (total, rate) => total + rate,
            0,
        ) * hourSeconds
    );
}

export function expectedProductionDayTailIterations(settings) {
    const consumerRates = productionDayConsumerRates(settings.headroom);
    const finalDayRate = consumerRates[consumerRates.length - 1];

    return (
        trapezoidIterations(
            finalDayRate,
            settings.drainRate,
            settings.drainRampUpSeconds,
        ) +
        settings.drainRate * settings.drainHoldSeconds +
        trapezoidIterations(
            settings.drainRate,
            0,
            settings.drainRampDownSeconds,
        )
    );
}

export function productionDayQueueIndex(iteration, role) {
    if (role === "producer") {
        const slot = (iteration * 137 + 450) % 500;
        return weightedIndex(slot, 500);
    }

    const slot = (iteration * 37 + 19) % 100;
    return weightedIndex(slot, 100);
}

export function productionDayPriority(iteration) {
    const block = Math.floor(iteration / 500);
    const slot = (iteration * 37 + block * 17) % 100;

    if (slot < 70) {
        return "HIGH";
    }
    if (slot < 90) {
        return "MEDIUM";
    }
    return "LOW";
}

export function productionDayCohort(iteration) {
    const slot = iteration % 500;

    if (slot === 0) {
        const faultIndex = Math.floor(iteration / 500);
        return {
            cohort: "fault",
            faultKind: FAULT_PATTERN[faultIndex % FAULT_PATTERN.length],
            processingSeconds:
                FAULT_PATTERN[faultIndex % FAULT_PATTERN.length] ===
                "expiry"
                    ? 5
                    : 1,
        };
    }
    if (slot <= 400) {
        return {
            cohort: "process_1s",
            faultKind: "none",
            processingSeconds: 1,
        };
    }
    if (slot <= 475) {
        return {
            cohort: "process_2s",
            faultKind: "none",
            processingSeconds: 2,
        };
    }
    if (slot <= 495) {
        return {
            cohort: "process_3s",
            faultKind: "none",
            processingSeconds: 3,
        };
    }

    return {
        cohort: "process_5s",
        faultKind: "none",
        processingSeconds: 5,
    };
}

export function expectedProductionDayCohorts(messageCount) {
    const counts = {
        process_1s: 0,
        process_2s: 0,
        process_3s: 0,
        process_5s: 0,
        retry_once: 0,
        retry_twice: 0,
        dead_letter: 0,
        expiry: 0,
    };

    const completeBlocks = Math.floor(messageCount / 500);
    const remainder = messageCount % 500;

    counts.process_1s = completeBlocks * 400;
    counts.process_2s = completeBlocks * 75;
    counts.process_3s = completeBlocks * 20;
    counts.process_5s = completeBlocks * 4;

    for (let block = 0; block < completeBlocks; block += 1) {
        counts[FAULT_PATTERN[block % FAULT_PATTERN.length]] += 1;
    }

    if (remainder > 0) {
        counts[FAULT_PATTERN[completeBlocks % FAULT_PATTERN.length]] += 1;
        counts.process_1s += Math.min(400, Math.max(0, remainder - 1));
        counts.process_2s += Math.min(
            75,
            Math.max(0, remainder - 401),
        );
        counts.process_3s += Math.min(
            20,
            Math.max(0, remainder - 476),
        );
        counts.process_5s += Math.min(
            4,
            Math.max(0, remainder - 496),
        );
    }

    return counts;
}

export function expectedProductionDayPriorities(messageCount) {
    const counts = {
        HIGH: Math.floor(messageCount / 500) * 350,
        MEDIUM: Math.floor(messageCount / 500) * 100,
        LOW: Math.floor(messageCount / 500) * 50,
    };
    const completeMessages = Math.floor(messageCount / 500) * 500;

    for (
        let iteration = completeMessages;
        iteration < messageCount;
        iteration += 1
    ) {
        counts[productionDayPriority(iteration)] += 1;
    }

    return counts;
}

export function expectedProductionDayQueues(messageCount) {
    const completeBlocks = Math.floor(messageCount / 500);
    const counts = {};

    for (const profile of PRODUCTION_DAY_QUEUE_PROFILES) {
        counts[profile.profile] = completeBlocks * profile.weight * 5;
    }

    const completeMessages = completeBlocks * 500;
    for (
        let iteration = completeMessages;
        iteration < messageCount;
        iteration += 1
    ) {
        const queue =
            PRODUCTION_DAY_QUEUE_PROFILES[
                productionDayQueueIndex(iteration, "producer")
            ];
        counts[queue.profile] += 1;
    }

    return counts;
}

export function expectedProductionDayDeliveryAccounting(messageCount) {
    const cohorts = expectedProductionDayCohorts(messageCount);
    const normalAcknowledgements =
        cohorts.process_1s +
        cohorts.process_2s +
        cohorts.process_3s +
        cohorts.process_5s;
    const faultMessages =
        cohorts.retry_once +
        cohorts.retry_twice +
        cohorts.dead_letter +
        cohorts.expiry;
    const secondAttempts =
        cohorts.retry_once +
        cohorts.retry_twice +
        cohorts.dead_letter;
    const thirdAttempts = cohorts.retry_twice + cohorts.dead_letter;
    const intentionalNoAcks =
        cohorts.retry_once +
        cohorts.retry_twice * 2 +
        cohorts.dead_letter * 3 +
        cohorts.expiry;

    return {
        acknowledgements: {
            first: normalAcknowledgements,
            second: cohorts.retry_once,
            third: cohorts.retry_twice,
            total:
                normalAcknowledgements +
                cohorts.retry_once +
                cohorts.retry_twice,
        },
        attempts: {
            first: messageCount,
            second: secondAttempts,
            third: thirdAttempts,
            total: messageCount + secondAttempts + thirdAttempts,
        },
        deadLetters: cohorts.dead_letter,
        faultMessages,
        intentionalNoAcks,
        previouslyDeliveredExpirations: cohorts.expiry,
    };
}
