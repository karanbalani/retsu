# Local infrastructure

The application runs on the host through Cargo. PostgreSQL and the optional observability stack run through Docker Compose.

## Prerequisites

- Rust toolchain
- Docker with Docker Compose

## Configuration overrides

The checked-in defaults are sufficient. To customize local ports or credentials:

```bash
cp .env.example .env
```

`.env` is intentionally excluded from version control.

## Start PostgreSQL
```bash
docker compose up -d --wait postgres
```

## Apply application-owned migrations:
```bash
cargo run -- migrate
```

## Start observability stack
```bash
docker compose --profile observability up -d --wait
```

## Run the API with trace export enabled
```bash
RETSU_TELEMETRY__TRACES__ENABLED=true cargo run -- api
```

## Local endpoints
- API: http://127.0.0.1:2424
- API metrics: http://127.0.0.1:2424/metrics
- Collector health: http://127.0.0.1:24243
- Tempo: http://127.0.0.1:24244
- Prometheus: http://127.0.0.1:24245
- Grafana: http://127.0.0.1:24246

**Grafana defaults to `admin` / `retsu_local` unless overridden in `.env`**

## Inpect services
```bash
docker compose ps
docker compose logs -f postgres
docker compose logs -f otel-collector tempo prometheus grafana
```

## Stop services
```bash
docker compose down
```
Named volumes are preserved.
Running `docker compose down --volumes` permanently deletes the local database,
metrics, traces, and Grafana state. Use it only when intentionally resetting the
entire local environment.
