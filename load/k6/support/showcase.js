export const SHOWCASE_START_RATE = 50;
export const SHOWCASE_TAIL_SECONDS = 120;
export const SHOWCASE_VERIFICATION_WINDOW_SECONDS = 15;

export const SHOWCASE_RATE_PATTERN = Object.freeze([
    Object.freeze({ durationSeconds: 6, target: 90 }),
    Object.freeze({ durationSeconds: 8, target: 140 }),
    Object.freeze({ durationSeconds: 10, target: 70 }),
    Object.freeze({ durationSeconds: 6, target: 150 }),
    Object.freeze({ durationSeconds: 8, target: 100 }),
    Object.freeze({ durationSeconds: 10, target: 50 }),
    Object.freeze({ durationSeconds: 6, target: 120 }),
    Object.freeze({ durationSeconds: 6, target: 50 }),
]);

export const SHOWCASE_QUEUE_PROFILES = Object.freeze([
    Object.freeze({
        profile: "hot-a",
        queueClass: "hot",
        weight: 35,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 10,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 180,
        }),
    }),
    Object.freeze({
        profile: "hot-b",
        queueClass: "hot",
        weight: 35,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 10,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 180,
        }),
    }),
    Object.freeze({
        profile: "warm-a",
        queueClass: "warm",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 10,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 180,
        }),
    }),
    Object.freeze({
        profile: "warm-b",
        queueClass: "warm",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 10,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 180,
        }),
    }),
    Object.freeze({
        profile: "fault",
        queueClass: "fault",
        weight: 10,
        settings: Object.freeze({
            visibilityTimeoutSeconds: 10,
            maxDeliveryAttempts: 3,
            defaultMessageTtlSeconds: 180,
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
        index < SHOWCASE_QUEUE_PROFILES.length;
        index += 1
    ) {
        upperBound +=
            (SHOWCASE_QUEUE_PROFILES[index].weight * denominator) /
            100;
        if (slot < upperBound) {
            return index;
        }
    }

    return SHOWCASE_QUEUE_PROFILES.length - 1;
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

export function showcaseActiveLoadMinutes(totalDurationMinutes) {
    return totalDurationMinutes - SHOWCASE_TAIL_SECONDS / 60;
}

export function showcaseProducerStages(activeLoadMinutes) {
    const stages = [];

    for (let minute = 0; minute < activeLoadMinutes; minute += 1) {
        for (const stage of SHOWCASE_RATE_PATTERN) {
            stages.push({
                duration: duration(stage.durationSeconds),
                target: stage.target,
            });
        }
    }

    return stages;
}

export function showcaseConsumerRate(rate, headroom) {
    const target = rate * headroom;

    if (!Number.isInteger(target)) {
        throw new Error(
            `consumer headroom ${headroom} does not produce an integer target for ${rate}/s`,
        );
    }

    return target;
}

export function showcaseConsumerStages(settings) {
    const stages = showcaseProducerStages(settings.activeLoadMinutes).map(
        (stage) => ({
            duration: stage.duration,
            target: showcaseConsumerRate(stage.target, settings.headroom),
        }),
    );

    stages.push(
        {
            duration: duration(settings.drainRampUpSeconds),
            target: settings.drainRate,
        },
        {
            duration: duration(settings.drainHoldSeconds),
            target: settings.drainRate,
        },
        {
            duration: duration(settings.drainRampDownSeconds),
            target: 0,
        },
    );

    return stages;
}

export function showcaseVerificationStartSeconds(settings) {
    const totalSeconds = settings.durationMinutes * 60;
    const drainSeconds =
        settings.drainRampUpSeconds +
        settings.drainHoldSeconds +
        settings.drainRampDownSeconds;
    const observationSeconds =
        SHOWCASE_TAIL_SECONDS -
        drainSeconds -
        SHOWCASE_VERIFICATION_WINDOW_SECONDS;

    if (observationSeconds < 60) {
        throw new Error(
            "showcase drain must leave at least one cleaner interval before verification",
        );
    }

    return totalSeconds - SHOWCASE_VERIFICATION_WINDOW_SECONDS;
}

function expectedPatternIterations(startRate, stages) {
    let previousRate = startRate;
    let iterations = 0;

    for (const stage of stages) {
        iterations += trapezoidIterations(
            previousRate,
            stage.target,
            stage.durationSeconds,
        );
        previousRate = stage.target;
    }

    return iterations;
}

export function expectedShowcaseEnqueues(activeLoadMinutes) {
    return (
        expectedPatternIterations(
            SHOWCASE_START_RATE,
            SHOWCASE_RATE_PATTERN,
        ) * activeLoadMinutes
    );
}

export function expectedShowcaseMainConsumerIterations(
    activeLoadMinutes,
    headroom,
) {
    const consumerStages = SHOWCASE_RATE_PATTERN.map((stage) => ({
        durationSeconds: stage.durationSeconds,
        target: showcaseConsumerRate(stage.target, headroom),
    }));

    return (
        expectedPatternIterations(
            showcaseConsumerRate(SHOWCASE_START_RATE, headroom),
            consumerStages,
        ) * activeLoadMinutes
    );
}

export function expectedShowcaseTailIterations(settings) {
    const finalRate = showcaseConsumerRate(
        SHOWCASE_RATE_PATTERN[SHOWCASE_RATE_PATTERN.length - 1].target,
        settings.headroom,
    );

    return (
        trapezoidIterations(
            finalRate,
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

export function showcaseQueueIndex(iteration, role) {
    if (role === "producer") {
        const slot = (iteration * 137 + 450) % 500;
        return weightedIndex(slot, 500);
    }

    const slot = (iteration * 37 + 19) % 100;
    return weightedIndex(slot, 100);
}

export function showcasePriority(iteration) {
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

export function showcaseCohort(iteration) {
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

export function expectedShowcaseCohorts(messageCount) {
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

export function expectedShowcasePriorities(messageCount) {
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
        counts[showcasePriority(iteration)] += 1;
    }

    return counts;
}

export function expectedShowcaseQueues(messageCount) {
    const completeBlocks = Math.floor(messageCount / 500);
    const counts = {};

    for (const profile of SHOWCASE_QUEUE_PROFILES) {
        counts[profile.profile] = completeBlocks * profile.weight * 5;
    }

    const completeMessages = completeBlocks * 500;
    for (
        let iteration = completeMessages;
        iteration < messageCount;
        iteration += 1
    ) {
        const queue =
            SHOWCASE_QUEUE_PROFILES[
                showcaseQueueIndex(iteration, "producer")
            ];
        counts[queue.profile] += 1;
    }

    return counts;
}

export function expectedShowcaseDeliveryAccounting(messageCount) {
    const cohorts = expectedShowcaseCohorts(messageCount);
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
