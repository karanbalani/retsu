# Local infrastructure

The Retsu application runs on the host through Cargo. PostgreSQL and the optional
monitoring services run through Docker Compose.

## Prerequisites

- Rust toolchain
- Docker with Docker Compose
- Just 1.45 or newer
- Bash

Verify the environment:

```bash
just doctor
```

## Configuration overrides

The checked-in defaults work without additional configuration.

To customize Docker Compose ports or credentials:

```bash
just env-init
```

The root `.env` file is intentionally excluded from version control.

It is used only by Docker Compose. Do not source it into the application
environment because its `RETSU_LOCAL_*` variables are not Retsu application
configuration fields.

## PostgreSQL

Start PostgreSQL and apply pending migrations:

```bash
just setup
```

Start PostgreSQL without applying migrations:

```bash
just db-up
```

Open a PostgreSQL shell:

```bash
just db-shell
```

Stop PostgreSQL while preserving its data:

```bash
just db-stop
```

To log database queries that take 250 milliseconds or longer, set this in
`.env`:

```dotenv
RETSU_LOCAL_POSTGRES_SLOW_QUERY_MS=250
```

Use `-1` to turn this logging off. Restart PostgreSQL after changing the value.

## Monitoring

Start PostgreSQL and all monitoring services:

```bash
just stack-up
```

`pg-exporter` provides PostgreSQL measurements, and `cadvisor` provides
container measurements. Prometheus collects both automatically.

Stop the monitoring services while preserving their data:

```bash
just observability-stop
```

Run the API and send its activity to the monitoring tools:

```bash
just api-observed
```

Run the queue workers and send their activity to the monitoring tools. Give the
second worker a different management port:

```bash
just worker-observed queue visibility-timeout-processor
RETSU_WORKER__MANAGEMENT__PORT=24250 \
  just worker-observed queue expired-message-cleaner
```

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
- PostgreSQL: 127.0.0.1:24240
- Collector health: http://127.0.0.1:24243
- Tempo: http://127.0.0.1:24244
- Prometheus: http://127.0.0.1:24245
- Grafana: http://127.0.0.1:24246
- Worker liveness: http://127.0.0.1:24247/health/live
- Worker readiness: http://127.0.0.1:24247/health/ready
- Worker metrics: http://127.0.0.1:24247/metrics
- PostgreSQL measurements: http://127.0.0.1:24248/metrics
- Container measurements: http://127.0.0.1:24249/metrics

Grafana defaults to `admin` / `retsu_local` unless overridden through `.env`.
