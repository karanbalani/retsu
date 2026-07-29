# Monitoring

The API and workers provide logs, Prometheus metrics, health checks, and optional trace export. The one-time migration role logs its work and exits.

## Process endpoints

| Process | Default address | Endpoints |
| --- | --- | --- |
| API | `127.0.0.1:2424` | `/health/live`, `/health/ready`, `/metrics` |
| Worker | `127.0.0.1:24247` | `/health/live`, `/health/ready`, `/metrics` |

`/health/live` confirms that the process is running. `/health/ready` also checks whether it can use PostgreSQL.

Do not send public traffic to worker management ports.

## Metrics

Retsu exposes measurements for:

- HTTP request rate, duration, active requests, and status;
- database pool waits, query duration, and outcomes;
- cache hits, misses, load duration, and outcomes;
- enqueue, acknowledge, expiry, dead-letter, and dead-letter purge events;
- ready and in-flight messages by queue and priority;
- oldest ready and in-flight message age;
- state-collection success, duration, and snapshot age.

Queue event measurements are recorded where the queue operation succeeds, not in an HTTP handler. This keeps API and worker behavior consistent.

The state-metrics collector supplies the current queue gauges. See [Queue state summaries](queue-state-rollups.md), [Queue metric limits](queue-metric-cardinality.md), and [State collector failover](queue-state-collector-leadership.md).

## Logs

Local host processes use readable `pretty` logs at `warn` level by default. Container processes should normally use `json`.

Example:

```console
RETSU_LOGGING__FORMAT=json \
RETSU_LOGGING__FILTER=info \
  retsu api
```

Each process writes its role, environment, version, and selected worker fields into its main activity span.

## Traces

Trace export is disabled by default:

```console
RETSU_TELEMETRY__TRACES__ENABLED=true \
RETSU_TELEMETRY__TRACES__ENDPOINT=http://collector:4317 \
  retsu api
```

The trace filter is separate from the log filter. `/metrics` requests are intentionally excluded from HTTP traces.

## Local monitoring stack

`just local-up` starts:

- Prometheus for metrics;
- Grafana for dashboards;
- Tempo for traces;
- the OpenTelemetry Collector for trace intake;
- PostgreSQL and PgBouncer exporters;
- cAdvisor for container measurements.

Open Grafana at <http://127.0.0.1:24246>.

The **Retsu Local Showcase** dashboard focuses on message flow, priorities, retries, expiry, and dead-letter cleanup:

<http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase>

The performance dashboard adds database, cache, container, and load-generator details:

<http://127.0.0.1:24246/d/retsu-performance/retsu-performance>

Run `just local-showcase` or one of the [load scenarios](load-testing.md) while a dashboard is open.

The [detailed local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) lists every local endpoint, retention setting, resource limit, and troubleshooting command.
