# Retsu documentation

Retsu is an observable, distributed priority queue. These guides explain how to use it, run it, and understand its code.

## Start here

- [Getting started](getting-started.md) — Start the complete local stack and try Retsu.
- [Queues and messages](queues.md) — Use every public queue API.
- [Message lifecycle](message-lifecycle.md) — Understand delivery, retries, expiry, and dead-letter storage.

## Run Retsu

- [Configuration](configuration.md) — Set ports, connections, caches, traces, and worker behavior.
- [Workers](workers.md) — Run the three background jobs.
- [Local development](local-development.md) — Work on Retsu with containers or the host Rust toolchain.
- [Deployment and releases](deployment.md) — Run the production image and understand its release tags.
- [Monitoring](observability.md) — Use health checks, metrics, logs, traces, and Grafana.
- [Load testing](load-testing.md) — Run the smoke, performance, and showcase scenarios.

## Understand the code

- [Architecture](architecture.md) — See the running processes and shared services.
- [Codebase guide](codebase-guide.md) — Understand dependency injection, modules, and where changes belong.
- [Caching](caching.md) — Understand the local queue-name and shared queue-details caches.
- [Queue state summaries](queue-state-rollups.md) — See how queue counts stay inexpensive to collect.
- [Queue metric limits](queue-metric-cardinality.md) — Keep per-queue metrics within a known memory budget.
- [State collector failover](queue-state-collector-leadership.md) — Keep one collector active while standby processes wait.

## Project guides

- [Contributing](https://github.com/karanbalani/retsu/blob/main/CONTRIBUTING.md)
- [Security policy](https://github.com/karanbalani/retsu/blob/main/SECURITY.md)
- [Detailed local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md)
