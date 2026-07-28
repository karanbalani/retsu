# Retsu k6 load generator

These scripts exercise the public queue API from outside the Kubernetes cluster.
External execution keeps load-generator CPU and network use separate from the
small cluster being measured.

Run tests only against an environment you own and are allowed to load test. The
defaults are intentionally small. Start with the smoke test, watch the API and
PostgreSQL, and raise rates gradually.

## Requirements

- k6 2.0.0 or later
- A reachable Retsu API
- A disposable database for benchmark series

The examples assume the API is available at `http://127.0.0.1:2424`.

## Correctness smoke test

The smoke test creates a unique queue, enqueues one message, verifies the
dequeued message ID, payload, and priority, acknowledges it using the returned
receipt handle, and confirms the queue is empty:

```bash
BASE_URL=http://127.0.0.1:2424 \
  k6 run load/k6/smoke.js
```

## Staged benchmark

Use a unique `RUN_ID` for each command if several tests may start in the same
millisecond.

Measure enqueue capacity while intentionally building backlog:

```bash
BASE_URL=https://retsu.example.com \
RUN_ID=enqueue-001 \
ENQUEUE_RATE=50 \
ENQUEUE_DURATION=2m \
QUEUE_COUNT=4 \
  k6 run --summary-export=enqueue-summary.json load/k6/enqueue.js
```

Measure dequeue and acknowledge drain rate from a prefilled queue set:

```bash
BASE_URL=https://retsu.example.com \
RUN_ID=consume-001 \
PREFILL_MESSAGES=5000 \
CONSUME_VUS=20 \
QUEUE_COUNT=4 \
  k6 run --summary-export=consume-summary.json load/k6/consume.js
```

Run steady producers and consumers independently:

```bash
BASE_URL=https://retsu.example.com \
RUN_ID=mixed-001 \
MIXED_PRODUCER_RATE=50 \
MIXED_CONSUMER_RATE=50 \
MIXED_DURATION=5m \
MIXED_PREFILL_MESSAGES=200 \
QUEUE_COUNT=4 \
  k6 run --summary-export=mixed-summary.json load/k6/mixed.js
```

Ramp to a steady rate, inject a spike, recover, and ramp down:

```bash
BASE_URL=https://retsu.example.com \
RUN_ID=saturation-001 \
SATURATION_START_RATE=10 \
SATURATION_RAMP_RATE=100 \
SATURATION_SPIKE_RATE=200 \
SATURATION_CONSUMER_RATIO=1 \
QUEUE_COUNT=4 \
  k6 run --summary-export=saturation-summary.json load/k6/saturation.js
```

Repeat each benchmark at least three times with the same application image,
replica count, cluster size, and database settings. Warm the environment before
recording comparison runs.

## Container execution

The host k6 binary is preferred. If it is unavailable, the pinned container can
run the same scripts against a remote API:

```bash
docker run --rm \
  --volume "$PWD/load/k6:/scripts:ro" \
  --env BASE_URL=https://retsu.example.com \
  grafana/k6:2.0.0 run /scripts/smoke.js
```

For an API running on Docker Desktop's host, set
`BASE_URL=http://host.docker.internal:2424`.

## Common settings

| Variable | Default | Purpose |
| --- | ---: | --- |
| `BASE_URL` | `http://127.0.0.1:2424` | API root without `/v1` |
| `RUN_ID` | Generated | Unique suffix for queues created by one run |
| `QUEUE_PREFIX` | `retsu-k6` | Prefix for generated queue names |
| `QUEUE_COUNT` | `1` | Queues spread across the workload |
| `PAYLOAD_BYTES` | `1024` | ASCII payload size, including its marker |
| `PRIORITY_MIX` | `20,60,20` | HIGH, MEDIUM, LOW integer weights |
| `MESSAGE_TTL_SECONDS` | `3600` | TTL used for every generated message |
| `VISIBILITY_TIMEOUT_SECONDS` | `30` | Visibility timeout for generated queues |
| `MAX_DELIVERY_ATTEMPTS` | `5` | Delivery limit for generated queues |
| `REQUEST_TIMEOUT` | `5s` | Per-request timeout |
| `SETUP_TIMEOUT` | `5m` | Queue creation and prefill timeout |
| `GRACEFUL_STOP` | `10s` | Time allowed for in-flight iterations |
| `EMPTY_DEQUEUE_SLEEP_MS` | `100` | Consumer pause after an empty dequeue |

Each script also accepts the rate, duration, VU, and prefill variables shown in
its example. `load/k6/support/config.js` contains the full list and safe bounds.

## Thresholds

The default thresholds fail a run when:

- HTTP or expected-status error rate reaches 1 percent.
- check success or lifecycle correctness falls to 99 percent or lower.
- p95 enqueue, dequeue, or acknowledge latency reaches 750 ms.

Override them with:

- `MAX_STATUS_ERROR_RATE`
- `MIN_CHECK_RATE`
- `MIN_LIFECYCLE_CORRECTNESS_RATE`
- `ENQUEUE_P95_MS`
- `DEQUEUE_P95_MS`
- `ACKNOWLEDGE_P95_MS`

A saturation run may fail latency thresholds by design. The first stage where
latency, errors, dropped iterations, or backlog grows without recovery marks a
capacity boundary.

## Result interpretation

Use these k6 metrics with the cluster dashboards:

| Metric | Meaning |
| --- | --- |
| `queue_operation_duration` | Latency tagged by create, prefill, enqueue, dequeue, or acknowledge |
| `status_errors` and `status_error_rate` | Responses outside the endpoint's expected statuses |
| `messages_enqueued` | Successful `201` responses with a retained message ID |
| `messages_dequeued` | Valid message deliveries |
| `messages_acknowledged` | Successful acknowledgements |
| `empty_dequeues` | Valid `204` responses; these are not HTTP or status failures |
| `lifecycles_started` | Messages returned to a consumer |
| `lifecycles_completed` | Returned messages successfully acknowledged |
| `lifecycle_correctness_rate` | Successful consumer lifecycles |
| `invalid_responses` | Expected status with a malformed response body |
| `dropped_iterations` | Offered load k6 could not schedule with the VU limit |

For enqueue-only tests, compare `messages_enqueued` with queue backlog. For
consume-only tests, compare elapsed time and `messages_acknowledged` with the
prefill count. For mixed and saturation tests, a widening difference between
enqueue and acknowledge counts indicates backlog growth.

## Repeatability and cleanup

Retsu does not currently provide queue deletion or purge endpoints. Every run
uses unique queue names, so repeated tests leave queues, messages, and possible
dead-letter records behind. Use a disposable database and reset it between
benchmark series. Do not compare long-running series against a database that
has accumulated data from earlier runs.
