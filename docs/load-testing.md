# Load testing

Retsu includes k6 scenarios for correctness checks, performance work, and the local product showcase.

Run tests only against an environment you own and are allowed to test. Start small and watch the API, database, and queue state while increasing load.

## Recommended local workflow

Start the complete stack:

```console
just local-up
```

Run a scenario in a temporary k6 container:

```console
just local-load smoke
just local-load enqueue
just local-load consume
just local-load mixed
just local-load saturation
```

The runner checks that the API, workers, Prometheus, and Grafana are ready. It removes the k6 container after the run and keeps the normal console summary.

## Scenarios

| Scenario | Purpose |
| --- | --- |
| `smoke` | Create a queue, enqueue, dequeue, acknowledge, and confirm it is empty |
| `enqueue` | Measure enqueue rate while intentionally building a backlog |
| `consume` | Prefill queues, then measure dequeue and acknowledgement drain rate |
| `mixed` | Run steady producers and consumers together |
| `saturation` | Ramp up, spike, recover, and ramp down to find a capacity boundary |
| `showcase` | Demonstrate priorities, retries, expiry, and dead-letter cleanup |

Every run creates queue names with a unique `RUN_ID`. Set one when a recognizable suffix is useful:

```console
RUN_ID=regression-001 just local-load mixed
```

## Showcase

Open the [Retsu Showcase dashboard](http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase), then run:

```console
just local-showcase
```

The default five-minute run applies active load for three minutes, then drains consumers, waits through a cleaner cycle, and checks final server state. Use a whole number from 5 through 20 to choose the total duration:

```console
just local-showcase 20
```

The workload sends a repeating 50–150 enqueue requests per second across five queues. Two queues receive most of the traffic. Priorities follow a 70/20/10 HIGH/MEDIUM/LOW mix. Controlled missing acknowledgements and short lifetimes create retries, expiry, and dead-letter records.

This is a product demonstration, not a capacity claim.

## Changing a run

Pass scenario settings on the same command:

```console
MIXED_PRODUCER_RATE=25 \
MIXED_CONSUMER_RATE=25 \
MIXED_DURATION=2m \
QUEUE_COUNT=4 \
  just local-load mixed
```

Common settings:

| Variable | Default | Purpose |
| --- | ---: | --- |
| `BASE_URL` | `http://127.0.0.1:2424` | API root without `/v1` |
| `RUN_ID` | Generated | Unique queue suffix |
| `QUEUE_PREFIX` | `retsu-k6` | Queue name prefix |
| `QUEUE_COUNT` | `1` | Number of queues |
| `PAYLOAD_BYTES` | `1024` | Generated payload size |
| `PRIORITY_MIX` | `20,60,20` | HIGH, MEDIUM, LOW weights |
| `MESSAGE_TTL_SECONDS` | `180` | Generated message lifetime |
| `VISIBILITY_TIMEOUT_SECONDS` | `10` | Generated queue visibility timeout |
| `MAX_DELIVERY_ATTEMPTS` | `3` | Generated queue delivery limit |
| `REQUEST_TIMEOUT` | `5s` | Per-request timeout |

Each scenario has additional rate, duration, consumer, and prefill settings in `load/k6/support/config.js`.

## Thresholds

The normal scenarios fail when:

- HTTP or unexpected-status errors reach 0.01 percent;
- check success or lifecycle correctness reaches 99 percent or lower;
- p95 enqueue, dequeue, or acknowledgement time reaches 750 milliseconds.

A saturation run may cross latency thresholds by design. The first stage where latency, errors, dropped work, or backlog grows without recovering marks a capacity boundary.

## Reading results

Use the k6 summary together with the Grafana dashboards:

- Compare enqueue and acknowledgement counts to see whether backlog grows.
- Treat empty dequeue responses as valid, not as request failures.
- Watch dropped iterations to see whether k6 could schedule the offered load.
- Check PostgreSQL, PgBouncer, cache, and container measurements before attributing a limit to the API.

Local k6 shares Docker capacity with Retsu and its supporting services. Use it for correctness, tuning, and repeatable comparisons on the same machine. Generate final environment capacity measurements from separate infrastructure.

## Direct k6 execution

The scripts can target a reachable API with k6 2.0.0 or later:

```console
BASE_URL=https://retsu.example.com \
RUN_ID=smoke-001 \
  k6 run load/k6/smoke.js
```

Use the files in `load/k6/` as executable test assets. Reader documentation stays in this guide.

## Cleanup

Retsu does not currently provide queue deletion or purge endpoints. Load runs leave their queues and any remaining messages in PostgreSQL. Use a disposable database and reset it between benchmark series.
