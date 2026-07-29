# Operations

Retsu uses one binary and container image for the API, migrations, and background workers. A complete deployment runs each role separately.

## Configure Retsu

Settings are loaded in this order:

1. Built-in defaults
2. `config/retsu.yaml`, when present
3. Environment variables

Later values override earlier ones. Pass `--config PATH` when a specific YAML file must be present:

```console
retsu --config /etc/retsu.yaml api
```

Environment variables start with `RETSU_` and use double underscores between YAML levels:

```console
RETSU_HTTP__PORT=3000
RETSU_DATABASE__URL=postgres://user:password@database:5432/retsu
RETSU_CACHE__DISTRIBUTED__URL=redis://cache:6379
RETSU_LOGGING__FORMAT=json
```

Common defaults are:

| Setting | Default |
| --- | --- |
| API address | `127.0.0.1:2424` |
| Worker management address | `127.0.0.1:24247` |
| Database pool size | 10 connections per process |
| Worker shutdown timeout | 30 seconds |
| Log format | `pretty` |
| Trace export | Disabled |

Bind to `0.0.0.0` inside a container. Keep connection URLs and other secrets in the deployment's secret manager, not in the repository.

See [Configuration](configuration.md) for the complete list, defaults, accepted ranges, and environment-variable format.

## Run the processes

Run migrations once before starting a new release:

```console
retsu migrate
```

Then run the API and all three workers as separate processes:

| Process | Command | Purpose |
| --- | --- | --- |
| API | `retsu api` | Serves queue requests |
| Expired-message cleaner | `retsu worker run queue expired-message-cleaner` | Removes messages after their lifetime and visibility timeout end |
| Dead-letter-message cleaner | `retsu worker run queue dead-letter-message-cleaner` | Removes dead-letter records after their retention period |
| State-metrics collector | `retsu worker run queue state-metrics-collector` | Refreshes ready, in-flight, and oldest-message measurements |

Starting one process does not start any other process. Several state collectors can run for failover, but only one collects at a time.

See [Workers](workers.md) for worker timing, management ports, and shutdown behavior.

## Deploy the image

Published images use an explicit calendar version:

```text
ghcr.io/karanbalani/retsu:YEAR.MONTH.RELEASE
```

Each release also has a `sha-<commit>` tag. There is no `latest` tag.

A safe rollout is:

1. Run migrations as a one-time job.
2. Start or update the API.
3. Start or update every worker.
4. Wait for each process to report ready.
5. Scrape metrics from the API and every worker.

The image runs as user and group `65532` and contains no shell or package manager. It supports Linux AMD64 and ARM64.

See [Deployment and releases](deployment.md) for image tags, commands, rollout details, and the release workflow.

## Check each process

The API and workers expose:

- `/health/live` to confirm that the process is running
- `/health/ready` to confirm that it can use PostgreSQL
- `/metrics` for Prometheus

The API uses its HTTP port. Each worker uses its management port, so workers on the same host need different ports. Do not expose worker management ports to public traffic.

Retsu writes structured logs and can export traces through OTLP. Enable trace export with:

```console
RETSU_TELEMETRY__TRACES__ENABLED=true \
RETSU_TELEMETRY__TRACES__ENDPOINT=http://collector:4317 \
  retsu api
```

The local stack includes Prometheus, Grafana, Tempo, and the OpenTelemetry Collector. Open Grafana at <http://127.0.0.1:24246>.

See [Monitoring](observability.md) for metrics, logs, traces, and dashboards. The [local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) lists every local port, resource limit, and troubleshooting command.

## Create a release

Maintainers create a calendar-version tag from a clean local `main` that matches `origin/main`:

```console
just release-tag 2026.7.0
```

The release workflow builds both image architectures, publishes version and commit tags, and creates a GitHub release.
