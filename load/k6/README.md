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
- A reachable Prometheus for showcase server-outcome verification
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

Run the local showcase with message processing, retries, expiry, and dead-letter
behavior:

```bash
SHOWCASE_DURATION_MINUTES=5 \
RUN_ID=showcase-001 \
  k6 run load/k6/showcase.js
```

The showcase repeats this deterministic one-minute enqueue wave:

```text
start 50 RPS
6s → 90, 8s → 140, 10s → 70, 6s → 150,
8s → 100, 10s → 50, 6s → 120, 6s → 50
```

That is 5,820 enqueues per minute, averaging 97 RPS while continuously moving
between 50 and 150 RPS every 6–10 seconds. Five queues receive
35/35/10/10/10 percent of producer traffic: two hot queues, two warm queues,
and one lifecycle/fault queue. Priorities independently follow
70/20/10 HIGH/MEDIUM/LOW, and each payload is a small five-field JSON object.

Every queue uses a 10-second visibility timeout, three maximum delivery
attempts, and a 180-second default message TTL. Normal messages explicitly use
that TTL. Intentional fault messages use shorter TTLs so retry, expiry, and
dead-letter behavior appears during the same run.

Consumers have 30 percent scheduling headroom. Work is deterministic:
80 percent takes one second, 15 percent takes two seconds, 4 percent takes
three seconds, and the final 1 percent is split between five-second work and
intentional missing acknowledgements. The latter exercises one retry, two
retries, expiry, and delivery-attempt exhaustion into the DLQ.
The local dead-letter cleaner retains those records for one hour.

The selected 5–20 minutes bounds the complete k6 run. Its final two minutes
contain a 32-second consumer drain, more than one full cleaner cycle of
observation, and a 15-second final server-state verification. The default
five-minute run therefore applies three minutes of active load and produces
exactly 17,460 enqueues before completing at five minutes. This is a product
demonstration, not a capacity benchmark.

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
just local-showcase
just local-showcase 20
```

`just local-showcase` runs for five minutes total. Pass any whole number from
5 through 20 to choose a longer run; values outside that range are rejected
before k6 starts.

Supply scenario overrides on the same command:

```bash
MIXED_PRODUCER_RATE=25 \
MIXED_CONSUMER_RATE=25 \
MIXED_DURATION=2m \
QUEUE_COUNT=4 \
  just local-load mixed
```

The local runner sends metrics to Prometheus and prints the normal k6 summary.
Showcase results appear in the
[Retsu Showcase dashboard](http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase).
The k6 container has no host port or persistent storage and is removed after
every run.

Allocate at least 8 CPUs and 8 GiB of memory to Docker for the complete local
stack and k6 runner. The Compose resource profile is fixed so local showcase
runs use consistent capacity.

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
| `PROMETHEUS_URL` | `http://127.0.0.1:24245` | Prometheus root used by showcase final verification |
| `RUN_ID` | Generated | Unique suffix for queues created by one run |
| `QUEUE_PREFIX` | `retsu-k6` | Queue prefix; `local-showcase` uses `retsu-showcase` |
| `QUEUE_COUNT` | `1` | Queues spread across the workload |
| `PAYLOAD_BYTES` | `1024` | ASCII payload size, including its marker |
| `PRIORITY_MIX` | `20,60,20` | HIGH, MEDIUM, LOW integer weights |
| `MESSAGE_TTL_SECONDS` | `180` | TTL used for every generated message |
| `VISIBILITY_TIMEOUT_SECONDS` | `10` | Visibility timeout for generated queues |
| `MAX_DELIVERY_ATTEMPTS` | `3` | Delivery limit for generated queues |
| `REQUEST_TIMEOUT` | `5s` | Per-request timeout |
| `SETUP_TIMEOUT` | `5m` | Queue creation and prefill timeout |
| `GRACEFUL_STOP` | `10s` | Time allowed for in-flight iterations |
| `EMPTY_DEQUEUE_SLEEP_MS` | `100` | Consumer pause after an empty dequeue |
| `SHOWCASE_DURATION_MINUTES` | `5` | Total showcase run time; whole number from 5 through 20 |

Each script also accepts the rate, duration, VU, and prefill variables shown in
its example. `load/k6/support/config.js` contains the full list and safe bounds.
These local scenarios explicitly create queues with the values above; they do
not change the API's fallback defaults when a caller omits queue settings.
The showcase defaults use 32/128 producer and 256/384 consumer
preallocated/maximum VUs. The final two minutes of the selected showcase
duration are reserved for drain, cleaner observation, and verification.
Showcase consumers do not apply `EMPTY_DEQUEUE_SLEEP_MS`:
their arrival-rate executor already caps dequeue polling, and sleeping after an
empty response would only retain VUs.

## Thresholds

The default thresholds fail a run when:

- HTTP or expected-status error rate reaches 0.01 percent.
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

The showcase uses these normal functional thresholds and also requires k6 to
schedule the complete offered workload without dropped iterations. It reports
the planned lifecycle accounting and final Prometheus state for visibility; it
does not turn those exact counts into benchmark gates.

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
