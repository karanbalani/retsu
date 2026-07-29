# Configuration

Retsu reads built-in defaults, then a YAML file, then environment variables. Later values override earlier ones.

By default, Retsu looks for `config/retsu.yaml`. The program can still use built-in defaults if that file is absent. Passing `--config PATH` makes the selected file required:

```console
retsu --config /etc/retsu.yaml api
```

Unknown YAML fields are rejected. This prevents misspelled settings from being silently ignored.

## Environment variables

Use the `RETSU_` prefix and double underscores between YAML levels:

```console
RETSU_HTTP__PORT=3000 retsu api
RETSU_LOGGING__FORMAT=json retsu api
RETSU_CACHE__DISTRIBUTED__URL=redis://cache.internal:6379 retsu api
```

Booleans and numbers are parsed automatically.

## General settings

| YAML setting | Default | Accepted value |
| --- | --- | --- |
| `environment` | `local` | `local`, `test`, `staging`, or `production` |
| `http.bind_address` | `127.0.0.1` | IP address |
| `http.port` | `2424` | 1–65,535 |
| `logging.filter` | `warn` | Non-empty log filter |
| `logging.format` | `pretty` | `pretty` or `json` |

Set `http.bind_address` to `0.0.0.0` when the API must accept traffic from outside its container.

## Metrics and traces

| YAML setting | Default | Accepted value |
| --- | ---: | --- |
| `telemetry.metrics.max_queues` | `10000` | 1–100,000 |
| `telemetry.traces.enabled` | `false` | Boolean |
| `telemetry.traces.filter` | `warn` | Non-empty trace filter |
| `telemetry.traces.endpoint` | `http://127.0.0.1:24241` | URL |
| `telemetry.traces.timeout_seconds` | `5` | 1–60 |

`max_queues` controls the number of queue-labelled metric series the process can retain. See [Queue metric limits](queue-metric-cardinality.md) before increasing it.

## Cache

| YAML setting | Default | Accepted value |
| --- | ---: | --- |
| `cache.in_memory.enabled` | `true` | Boolean |
| `cache.in_memory.regions.queue_names.max_entries` | `10000` | 1–1,000,000 |
| `cache.in_memory.regions.queue_names.max_capacity_bytes` | `8388608` | 1–4,294,967,295 |
| `cache.distributed.enabled` | `true` | Boolean |
| `cache.distributed.url` | `redis://127.0.0.1:24251` | URL |
| `cache.distributed.connection_timeout_milliseconds` | `500` | 1–10,000 |
| `cache.distributed.command_timeout_milliseconds` | `20` | 1–10,000 |

The in-memory and distributed layers can be disabled independently. PostgreSQL remains the source of truth. See [Caching](caching.md).

## Database

| YAML setting | Default | Accepted value |
| --- | ---: | --- |
| `database.url` | `postgres://retsu:retsu_local@127.0.0.1:24240/retsu` | PostgreSQL URL |
| `database.max_connections` | `10` | 1 or more |
| `database.acquire_timeout_seconds` | `5` | 5–60 |

Every API or worker process creates its own connection pool. Size the complete deployment, not only one process.

## Worker process

| YAML setting | Default | Accepted value |
| --- | ---: | --- |
| `worker.shutdown_timeout_seconds` | `30` | 1–300 |
| `worker.management.bind_address` | `127.0.0.1` | IP address |
| `worker.management.port` | `24247` | 1–65,535 |

## Queue workers

| YAML setting | Default | Accepted value |
| --- | ---: | --- |
| `worker.queue.dead_letter_message_cleaner.retention_seconds` | `1209600` | 3,600–31,536,000 |
| `worker.queue.dead_letter_message_cleaner.processing_interval_seconds` | `60` | 5–3,600 |
| `worker.queue.dead_letter_message_cleaner.batch_size` | `500` | 1–10,000 |
| `worker.queue.dead_letter_message_cleaner.saturated_batch_delay_milliseconds` | `50` | 1–5,000 |
| `worker.queue.expired_message_cleaner.processing_interval_seconds` | `60` | 5–3,600 |
| `worker.queue.expired_message_cleaner.batch_size` | `500` | 1–10,000 |
| `worker.queue.expired_message_cleaner.saturated_batch_delay_milliseconds` | `50` | 1–5,000 |
| `worker.queue.state_metrics_collector.collection_interval_seconds` | `15` | 5–3,600 |
| `worker.queue.state_metrics_collector.leadership_retry_interval_seconds` | `15` | 5–300 |

The batch delay is used when a cleaner fills its batch and probably has more work. The normal interval is used after a smaller batch.

## Secrets

Connection URLs can contain credentials. Supply production secrets through the runtime environment or a protected configuration file. Do not commit them.

The production container does not include `config/retsu.yaml`; mount a file or provide environment variables when running the image.
