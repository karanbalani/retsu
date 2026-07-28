import { config } from "./config.js";

function maximumRateThreshold(value) {
    return value === 0 || value === 1 ? `rate<=${value}` : `rate<${value}`;
}

function minimumRateThreshold(value) {
    return value === 0 || value === 1 ? `rate>=${value}` : `rate>${value}`;
}

export function thresholdsFor(operations, includeLifecycle = false) {
    const thresholds = {
        checks: [minimumRateThreshold(config.minimumCheckRate)],
        http_req_failed: [
            maximumRateThreshold(config.maximumStatusErrorRate),
        ],
        status_error_rate: [
            maximumRateThreshold(config.maximumStatusErrorRate),
        ],
    };

    const p95ByOperation = {
        enqueue: config.enqueueP95Ms,
        dequeue: config.dequeueP95Ms,
        acknowledge: config.acknowledgeP95Ms,
    };

    for (const operation of operations) {
        thresholds[`queue_operation_duration{operation:${operation}}`] = [
            `p(95)<${p95ByOperation[operation]}`,
        ];
    }

    if (includeLifecycle) {
        thresholds.lifecycle_correctness_rate = [
            minimumRateThreshold(config.minimumLifecycleCorrectnessRate),
        ];
    }

    return thresholds;
}

export function saturationStages(multiplier = 1) {
    const target = (rate) => Math.max(1, Math.round(rate * multiplier));

    return [
        {
            duration: config.saturationWarmupDuration,
            target: target(config.saturationStartRate),
        },
        {
            duration: config.saturationRampDuration,
            target: target(config.saturationRampRate),
        },
        {
            duration: config.saturationHoldDuration,
            target: target(config.saturationRampRate),
        },
        {
            duration: config.saturationSpikeDuration,
            target: target(config.saturationSpikeRate),
        },
        {
            duration: config.saturationRecoveryDuration,
            target: target(config.saturationRampRate),
        },
        {
            duration: config.saturationRampDownDuration,
            target: 0,
        },
    ];
}
