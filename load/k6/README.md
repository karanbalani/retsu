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
- A reachable Prometheus for production-day server-outcome verification
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

Run a compressed production day with message processing, retries, expiry, and
dead-letter behavior:

```bash
RUN_ID=production-day-001 \
  k6 run --summary-export=production-day-summary.json \
  load/k6/production-day.js
```

The 24 one-minute targets are:

```text
50, 50, 50, 50, 50, 50, 50, 150, 250, 100, 90, 100,
150, 250, 120, 100, 90, 50, 50, 50, 50, 150, 250, 50
```

They total 2,400 requests per second across the compressed hours, which is
exactly 144,000 enqueues and a 100/s day average. A one-second transition and
59-second hold model each hour without changing the total. Five queues receive
35/35/10/10/10 percent of producer traffic. Priorities use an independent
70/20/10 HIGH/MEDIUM/LOW schedule, and the payload is a small five-field JSON
object.

Consumers run with 30 percent headroom and simulate 80 percent one-second,
15 percent two-second, 4 percent three-second, and 0.8 percent five-second
successful work. The final 0.2 percent is a deterministic 288-message fault
cohort: 160 acknowledge on delivery two, 64 on delivery three, 32 exhaust
three attempts into dead-letter storage, and 32 expire after being delivered.
Deliberate missing acknowledgements have separate bounded metrics and do not
count as service errors.

The full run expects 187,200 day consumer iterations plus a bounded 9,945
iteration drain. The exact successful enqueue distribution is:

| Dimension | Expected messages |
| --- | ---: |
| HIGH / MEDIUM / LOW | 100,800 / 28,800 / 14,400 |
| hot-a / hot-b | 50,400 / 50,400 |
| warm-a / warm-b / fault | 14,400 / 14,400 / 14,400 |
| process 1s / 2s / 3s / 5s | 115,200 / 21,600 / 5,760 / 1,152 |
| fault cohort | 288 |

Consumers must observe 144,000 first, 256 second, and 96 third delivery
attempts. They deliberately omit 416 acknowledgements and must complete
143,936 acknowledgements. The server must report 32 dead letters, 32
previously delivered expirations, no never-delivered expirations, and no ready
or in-flight messages for the five run queues.

Including five queue creations, the expected public queue API request count is
485,086. The local run also makes eight read-only Prometheus queries during
final verification, for 485,094 total k6 HTTP requests. A separate scenario
starts after the active drain and keeps k6
scheduled through the 75-second cleaner wait; it then checks the existing
server counters and final queue state.

The ten-second first-delivery or oldest-message SLO is intentionally deferred.
This scenario does not claim or gate it.

Repeat each benchmark at least three times with the same application image,
replica count, cluster size, and database settings. Warm the environment before
recording comparison runs.

## Container execution

For the complete local stack, use the service-local Compose runner. It checks
the runtime and observability services before creating a disposable k6
container:

```bash
just local-load smoke
just local-load enqueue
just local-load consume
just local-load mixed
just local-load saturation
just local-load production-day
```

Supply scenario overrides on the same command:

```bash
MIXED_PRODUCER_RATE=25 \
MIXED_CONSUMER_RATE=25 \
MIXED_DURATION=2m \
QUEUE_COUNT=4 \
  just local-load mixed
```

The local runner sends metrics to Prometheus and prints the normal k6 summary.
Its results appear in the load row of the provisioned Retsu Performance
dashboard. The k6 container has no host port or persistent storage and is
removed after every run.

The pinned container can also run the scripts directly against a remote API:

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
| `PROMETHEUS_URL` | `http://127.0.0.1:24245` | Prometheus root used by production-day final verification |
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
The production-day defaults use 256/512 producer and 768/1,024 consumer
preallocated/maximum VUs. `PRODUCTION_DAY_HOUR_SECONDS` can compress a
development validation further, but the standard result is comparable only at
the default 60 seconds. Production-day consumers do not apply
`EMPTY_DEQUEUE_SLEEP_MS`: their arrival-rate executor already caps dequeue
polling at the configured rate, and sleeping would only retain VUs.

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

The production-day scenario is stricter: unexpected HTTP and status errors
must not exceed 0.01 percent, checks and lifecycle correctness must remain at
least 99.99 percent, and dropped iterations must be zero. It also fails unless:

- all 144,000 planned messages enqueue successfully with the exact priority,
  queue, processing, and fault distributions above;
- delivery attempts are exactly 144,000 / 256 / 96 for attempts one / two /
  three;
- intentional missing acknowledgements total exactly 416 with the planned
  retry, dead-letter, and expiry split;
- successful acknowledgements total exactly 143,936 at the expected attempts;
- existing server metrics report exactly 32 dead letters and 32 previously
  delivered expirations, no never-delivered expirations, 15 queue/priority
  state series, a successful snapshot no older than 30 seconds, and a fully
  drained queue set.

The first-delivery-age objective remains deferred and is not part of these
failure criteria.

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

Running k6 inside the same local Docker VM as the system under test affects
absolute capacity measurements. Use the local runner for functional checks,
tuning, and repeatable regressions. Generate final cloud benchmark traffic from
a separate machine or instance.
