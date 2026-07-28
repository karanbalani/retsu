# Queue metric cardinality

This guide explains why Retsu configures metric cardinality in the application and how to choose a queue budget.

**Cardinality** is the number of distinct label combinations produced by one metric. A queue identity and priority together create one combination.

Related guides:

- [Queue state rollups](queue-state-rollups.md) explains how the underlying state counts are maintained.
- [Queue state collector leadership](queue-state-collector-leadership.md) explains which worker exports state gauges.

## The problem

State gauges have one series for each queue and priority:

```text
series per queue-and-priority metric = queues × 3
```

The Rust OpenTelemetry SDK defaults to 2,000 combinations per instrument. A metric with three priorities reaches that limit at roughly 667 queues.

After the limit is reached, additional combinations can be combined into an `otel.metric.overflow` series. Per-queue dashboards then lose the queue identities that overflowed.

## Why this is application configuration

Retsu's current metrics path is:

```text
Rust OpenTelemetry SDK → Retsu /metrics → Prometheus
```

Prometheus directly scrapes the API and worker management endpoints. The local OpenTelemetry Collector handles traces; it is not between Retsu and Prometheus for application metrics.

The Rust SDK applies its cardinality limit while it aggregates measurements in the process. If it has already collapsed queue labels into overflow, a downstream Prometheus or Collector setting cannot recover them.

The application must therefore configure the SDK view before instruments are created.

## Configuration

Retsu exposes the expected queue budget:

```yaml
telemetry:
  metrics:
    max_queues: 10000
```

The environment override is:

```bash
RETSU_TELEMETRY__METRICS__MAX_QUEUES=20000
```

Accepted values are 1 through 100,000.

This is a supported-capacity setting, not a request-time switch. Increasing it allows the SDK and monitoring system to retain more time series, which consumes more memory.

## Per-instrument limits

Retsu derives a limit from the labels each instrument can produce:

| Instrument shape | Fixed combinations per queue | SDK limit |
| --- | ---: | ---: |
| Queue and priority | 3 | `max_queues × 3` |
| Queue and delivery history | 2 | `max_queues × 2` |
| Queue only | 1 | `max_queues` |

Queue-and-priority instruments:

- `queue.messages.enqueued` (`queue.id`)
- `queue.messages.ready`
- `queue.messages.in_flight`
- `queue.oldest_ready_message.age`
- `queue.oldest_in_flight_message.age`

The state instruments use `queue.name`.

Queue-and-delivery-history instrument:

- `queue.messages.expired`

Queue-only instruments:

- `queue.messages.acknowledged` (`queue.id`)
- `queue.messages.requeued`
- `queue.messages.dead_lettered`

The retry and dead-letter worker instruments use `queue.name`.

For the default of 10,000 queues, each queue-and-priority instrument can retain 30,000 combinations.

Collection-health metrics have no queue label and keep the SDK default.

## Why not use one large limit for every metric?

The SDK allocates aggregation storage based on the configured limit. Giving a queue-only counter the same `max_queues × 3` budget as a priority metric would reserve capacity that it can never legitimately use.

Per-instrument multipliers keep the supported queue count consistent without inflating every instrument.

## Safe and unsafe labels

Good labels have a bounded and understood set of values:

- queue name;
- queue ID;
- `HIGH`, `MEDIUM`, or `LOW`;
- the two expiry delivery-history values;
- success or error outcomes.

Never attach per-message values:

- message ID;
- receipt handle;
- payload;
- arbitrary error text;
- request ID.

Those values can create a new series for every operation and exhaust memory regardless of the queue budget.

## Choosing `max_queues`

Choose a value above the number of distinct queues one process is expected to observe.

Consider:

- current queue count;
- planned growth before the next deployment;
- whether queue deletion and name churn will be introduced;
- memory available to the application;
- time-series limits and retention in Prometheus.

The default 10,000 is a capacity target, not proof that every deployment can store every resulting series cheaply. A state collector at that limit can export four state gauges with up to 30,000 series each.

If the supported queue count changes, update the setting and capacity test together.

## Failure signal

Dashboards should alert if a queue metric contains:

```text
otel.metric.overflow="true"
```

That means the real label count exceeded the configured budget or a supposedly bounded label gained unexpected values.

Raising the limit can restore future measurements, but it does not repair historical series that were already combined.

## Main implementation files

- `config/retsu.yaml` defines the default queue budget.
- `src/configuration/schema.rs` validates it.
- `src/observability/metrics/mod.rs` installs the OpenTelemetry views before queue instruments are created.
