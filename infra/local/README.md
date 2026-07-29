# Local infrastructure

The complete local stack runs through Docker Compose. The migration job, API,
and all three queue workers use the same hardened production image.

## Prerequisites

- Docker with Docker Compose 2.20.0 or newer
- Just 1.45 or newer
- Bash
- curl

Verify the environment:

```bash
just doctor
```

Rust and SQLx are needed only for host-based application development. Verify
that additional toolchain with `just doctor-host`.

## Configuration overrides

The checked-in defaults work without additional configuration. They build the
production Dockerfile as `retsu-local:dev`, bind every host port to loopback,
and let the complete-stack command enable trace export after the observability
services are ready.

To customize Docker Compose ports or credentials:

```bash
just env-init
```

The root `.env` file is intentionally excluded from version control.

It is used only by Docker Compose. Do not source it into the application
environment because its `RETSU_LOCAL_*` variables are not Retsu application
configuration fields.

PostgreSQL credentials are embedded in local connection URLs. Keep the database
name and application and monitoring usernames and passwords URI-safe by using
only letters, numbers, `.`, `_`, `~`, or `-`.

The primary PostgreSQL user, database, and password and the Grafana admin
credentials initialize their persisted data on first startup. Changing those
values in `.env` does not rewrite existing PostgreSQL or Grafana data. Use the
confirmed, destructive `just stack-reset` command after changing them, or keep
the original values. The monitoring role password is reapplied by
`just local-up`.

Every command uses `infra/local/compose.yaml` as the canonical Compose entry
point. Docker Compose loads the root `.env` when it is present. Without it,
Compose uses the `${...:-...}` defaults in the service files, while the k6
scripts use their JavaScript defaults. `.env.example` is a copyable template
used by `just env-init`, not an automatic fallback. Compose configuration lives
beside the service it configures:

```text
infra/local/
├── compose.yaml
├── retsu/compose.yaml
├── postgres/compose.yaml
├── pgbouncer/compose.yaml
├── dragonfly/compose.yaml
├── pg-exporter/compose.yaml
├── prometheus/compose.yaml
├── cadvisor/compose.yaml
├── grafana/compose.yaml
├── tempo/compose.yaml
├── otel-collector/compose.yaml
└── k6/compose.yaml
```

The stable Compose project name remains `retsu`, preserving the existing
`retsu_*` data volumes created by the former root Compose file.

## Complete local stack

Build the production image without starting services:

```bash
just local-build
```

Build and start PostgreSQL, PgBouncer, Dragonfly, migrations, the API, all three
queue workers, and the existing observability stack:

```bash
just local-up
```

The command starts Tempo and the OpenTelemetry Collector, enables OTLP trace
export for the API and all three workers, and waits for the complete stack to
become ready. It then verifies fresh Retsu, PostgreSQL, PgBouncer, and container
measurements, including database and pooler backend health, before printing the
API and Grafana addresses.

The older tracing command remains as a compatibility alias for `local-up`:

```bash
just local-up-tracing
```

Both commands bring up the same trace-enabled local stack.

Verify readiness again or inspect every service:

```bash
just local-ready
just local-status
```

Validate the local configuration and load scripts without starting the stack:

```bash
just local-validate
```

Stop all services without deleting PostgreSQL, Prometheus, Tempo, or Grafana
data:

```bash
just local-stop
```

The image name, loopback ports, and log filter can be changed in `.env`. Local
resource and database-capacity boundaries are fixed in Compose so every run
uses the same test profile. All Retsu roles still use the same image.

The local stack keeps queue lifecycle behavior short enough to observe during
development:

- Local k6 scenarios create queues with a 10-second visibility timeout, three
  maximum delivery attempts, and a 180-second default message TTL.
- The expired-message cleaner checks every 60 seconds in batches of 500.
- Dead letters are retained for one hour from `dead_lettered_at`; the
  dead-letter cleaner checks every 60 seconds in batches of 500.
- Cleaner passes pause for 50 milliseconds between saturated batches.

These values are explicit local Compose and k6 settings. They do not change
Retsu's application fallback defaults for callers that omit queue settings.
The local stack also bounds queue metric cardinality at 1,000 queues and the
queue-name cache at 1,000 entries or 2 MiB.

## Production-like observability

The normal local stack preserves all Retsu application metrics and useful
PostgreSQL and container diagnostics. Collection is bounded rather than
disabled:

- Retsu and container measurements are collected every 10 seconds.
- PostgreSQL and PgBouncer measurements are collected every 15 seconds.
- Prometheus self-measurements are collected every 30 seconds.
- Prometheus keeps up to 24 hours or 512 MB, whichever boundary is reached
  first.
- PostgreSQL connections, transactions, locks, WAL, table, index,
  `pg_stat_statements`, and key `pg_stat_io` measurements remain available.
  Exporter cache bookkeeping, low-value `pg_stat_io` writeback/extension
  families, and I/O roles that cannot be active in this standalone server are
  not stored.
- cAdvisor collects CPU, throttling, memory, limits, OOM, filesystem, and
  network measurements for the stable `retsu` Compose project.
- Core performance and monitoring containers have explicit CPU, memory, PID,
  and Docker log rotation limits.

For the complete workload, allocate at least 8 CPUs and 8 GiB of memory to
Docker. The active limits total 12.3 CPUs and about 6.4 GiB while k6 runs; they
are ceilings rather than reservations, but Docker still needs room for its VM
and filesystem cache.

| Service | CPUs | Memory | PIDs |
| --- | ---: | ---: | ---: |
| Migration job (one-shot) | 0.25 | 128 MiB | 64 |
| API | 2.00 | 512 MiB | 128 |
| PostgreSQL | 3.00 | 2 GiB | 256 |
| PgBouncer | 1.00 | 128 MiB | 64 |
| PgBouncer exporter | 0.10 | 64 MiB | 32 |
| Dragonfly | 0.50 | 384 MiB | 64 |
| Expired-message cleaner | 0.25 | 128 MiB | 64 |
| Dead-letter-message cleaner | 0.25 | 128 MiB | 64 |
| State-metrics collector | 0.25 | 128 MiB | 64 |
| Prometheus | 0.75 | 512 MiB | 128 |
| Grafana | 0.75 | 512 MiB | 192 |
| PostgreSQL exporter | 0.25 | 192 MiB | 64 |
| cAdvisor | 0.35 | 192 MiB | 128 |
| OpenTelemetry Collector | 0.35 | 192 MiB | 128 |
| Tempo | 0.50 | 384 MiB | 128 |
| k6, while running | 2.00 | 1 GiB | 256 |

Dragonfly's 384 MiB container limit leaves runtime overhead above its 256 MiB
cache cap.

Docker rotates each service log after 10 MiB and keeps two files. These
limits are test boundaries, not production sizing recommendations.

The API uses a bounded SQLx pool of 50 client connections through PgBouncer in
transaction mode. PgBouncer accepts at most 128 clients and uses 40 normal plus
10 reserve PostgreSQL connections, with a hard per-database and per-user cap of
50. Migrations use two direct connections and each worker uses two direct
connections. PostgreSQL monitoring also connects directly;
the state collector therefore retains the physical session required by its
advisory lock. PostgreSQL accepts at most 128 connections, leaving explicit
headroom around all bounded local pools.

Prometheus collects PgBouncer client waits, active and idle backend
connections, pool capacity, transaction and query rates, and exporter health.
The exporter uses the existing stats-only PgBouncer access and is reachable
only inside the Compose networks.

The cAdvisor container is privileged because Docker container resource
measurements require host cgroup and runtime access. Do not use this local
configuration on a shared or untrusted Docker host.

Tempo and the OpenTelemetry Collector run with the complete local stack, so the
API and all three workers export traces to a healthy provisioned datasource.
Tempo's metrics generator remains disabled.
Retsu and PostgreSQL metrics already provide request and database latency
signals; generating the same measurements from spans would duplicate series
and processing. Trace search and Prometheus exemplar links are available for
local investigation.

### Showcase dashboard

Grafana provisions the **Retsu Local Showcase** dashboard automatically at:

```text
http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase
```

Start the showcase and keep this dashboard open to watch Retsu accept,
prioritise, deliver, retry, expire, and clean messages in real time. The main
product story stays open; database pools, cache behavior, and runtime
diagnostics are available in collapsed sections when deeper investigation is
useful. Use the queue and priority filters to focus the view without changing
the underlying Prometheus queries.

## Local load generation

Start the complete stack, then run the default correctness smoke test:

```bash
just local-up
just local-load
```

Select a scenario with the same command:

```bash
just local-load smoke
just local-load enqueue
just local-load consume
just local-load mixed
just local-load saturation
just local-load showcase
```

Each invocation checks that the API, workers, Prometheus, and Grafana are ready
before starting a disposable k6 2.0.0 container. k6 reaches the API only through
the Compose network, streams bounded trend statistics to Prometheus, keeps its
console summary, and is removed when the run ends.

`showcase` runs the production-shaped local workload documented in [Local development](../../docs/local-development.md#run-load-tests). It uses five queues, continuously varies enqueue demand, keeps intentional retry/expiry/DLQ outcomes separate from service errors, and leaves consumers running through a bounded drain. A separate verification scenario keeps the run alive for the cleaner wait, then checks the existing dead-letter, expiry, ready, and in-flight metrics for that run's unique queues.

The command fails on more than 0.01 percent unexpected request errors, any
dropped iteration, a mismatch in the exact enqueue/priority/queue/cohort,
attempt, retry, no-ack, or acknowledgement totals, or an incomplete server-side
drain. The first-delivery-age objective is intentionally deferred and is not
claimed by this local scenario.

The scenario settings from [Local development](../../docs/local-development.md#run-load-tests) can be supplied for one command:

```bash
ENQUEUE_RATE=25 ENQUEUE_DURATION=2m QUEUE_COUNT=4 \
  just local-load enqueue
```

Every command generates a unique run identifier. Set `RUN_ID` explicitly when
you need a recognizable queue suffix:

```bash
RUN_ID=regression-001 just local-load mixed
```

The Retsu Local Showcase dashboard is designed around that scenario. Select the
time window covering a completed run because k6 marks its series stale when the
one-shot container exits.

Retsu does not provide queue deletion or purge endpoints. Runs use fresh queue
names but leave their queues and any remaining messages in PostgreSQL. Reset the
disposable stack data between measured benchmark series rather than comparing a
clean database with one that has accumulated earlier load.

k6 shares the local Docker VM with Retsu, PostgreSQL, and the monitoring tools.
This setup is suitable for correctness checks, tuning, and regression
comparisons on the same machine. Run k6 from separate infrastructure for final
cloud capacity measurements.

## PostgreSQL and distributed cache

Start PostgreSQL and Dragonfly, then apply pending migrations:

```bash
just setup
```

Start PostgreSQL and Dragonfly without applying migrations:

```bash
just db-up
```

Open a PostgreSQL shell:

```bash
just db-shell
```

Inspect the API pooler's live client/backend allocation with `SHOW POOLS` and
its transaction totals with `SHOW STATS`:

```bash
docker compose --file infra/local/compose.yaml exec pgbouncer sh -c \
  'PGPASSWORD="$DB_PASSWORD" exec psql --host 127.0.0.1 --port 6432 --username "$DB_USER" --dbname pgbouncer --command "SHOW POOLS;"'
docker compose --file infra/local/compose.yaml exec pgbouncer sh -c \
  'PGPASSWORD="$DB_PASSWORD" exec psql --host 127.0.0.1 --port 6432 --username "$DB_USER" --dbname pgbouncer --command "SHOW STATS;"'
```

Stop PostgreSQL and Dragonfly while preserving PostgreSQL data:

```bash
just db-stop
```

To log database queries that take 250 milliseconds or longer, set this in
`.env`:

```dotenv
RETSU_LOCAL_POSTGRES_SLOW_QUERY_MS=250
```

Use `-1` to turn this logging off. Restart PostgreSQL after changing the value.

Temporary files of at least 1 MiB are logged by default. Change the threshold
with `RETSU_LOCAL_POSTGRES_LOG_TEMP_FILES_KB`; use `-1` to disable temporary-file
logging or `0` to log every temporary file.

## Monitoring

Start PostgreSQL and all monitoring services:

```bash
just stack-up
```

`pg-exporter` provides PostgreSQL measurements, `pgbouncer-exporter` provides
pooler measurements, and `cadvisor` provides container measurements. Prometheus
collects all three automatically.

Stop the monitoring services while preserving their data:

```bash
just observability-stop
```

This stops the PgBouncer exporter but leaves PgBouncer itself running because
the local API uses it.

The complete local stack runs the API and workers automatically. The existing
host-based commands remain available for application development:

```bash
just api-observed
```

Run the queue workers and send their activity to the monitoring tools. Give
each worker a different management port:

```bash
just worker-observed queue expired-message-cleaner
RETSU_WORKER__MANAGEMENT__PORT=24253 \
  just worker-observed queue dead-letter-message-cleaner
RETSU_WORKER__MANAGEMENT__PORT=24252 \
  just worker-observed queue state-metrics-collector
```

See [queue state collector leadership](../../docs/queue-state-collector-leadership.md)
to run active and standby collector processes.

Inspect running and stopped services:

```bash
just stack-status
```

Follow service logs:

```bash
just logs postgres
just logs otel-collector
just logs tempo
just logs prometheus
just logs pg-exporter
just logs cadvisor
just logs grafana
```

Stop and remove containers while preserving persisted data:

```bash
just stack-down
```

Delete all local PostgreSQL, Prometheus, Tempo, and Grafana data:

```bash
just stack-wipe
```

Delete all data, recreate the complete stack, and apply migrations:

```bash
just stack-reset
```

Both destructive commands require interactive confirmation.

## Database migrations

Install the pinned SQLx CLI:

```bash
just sqlx-install
```

Create a forward-only migration:

```bash
just migration-new create_queues_and_messages
```

Migration descriptions must use lowercase `snake_case`. Migration files must
never be created or renamed manually.

Apply migrations:

```bash
just migrate
```

## Local endpoints

- API: http://127.0.0.1:2424
- API metrics: http://127.0.0.1:2424/metrics
- Expired-message cleaner health: http://127.0.0.1:24247/health/ready
- Expired-message cleaner metrics: http://127.0.0.1:24247/metrics
- Dead-letter-message cleaner health: http://127.0.0.1:24253/health/ready
- Dead-letter-message cleaner metrics: http://127.0.0.1:24253/metrics
- State-metrics collector health: http://127.0.0.1:24252/health/ready
- State-metrics collector metrics: http://127.0.0.1:24252/metrics
- PostgreSQL: 127.0.0.1:24240
- PgBouncer: 127.0.0.1:24250
- Distributed cache: 127.0.0.1:24251
- OpenTelemetry collector health: http://127.0.0.1:24243
- Tempo: http://127.0.0.1:24244
- Prometheus: http://127.0.0.1:24245
- Grafana: http://127.0.0.1:24246
- PostgreSQL measurements: http://127.0.0.1:24248/metrics
- Container measurements: http://127.0.0.1:24249/metrics

Grafana defaults to `admin` / `retsu_local` unless overridden through `.env`.
