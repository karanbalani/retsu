# Documentation

## Start here

- [Getting started](getting-started.md) — Set up and run Retsu.
- [Queues and messages](queues.md) — Create and configure a queue, add messages, get the next one, and mark it as complete.

## Understand the code

- [Architecture](architecture.md) — See how the API, workers, caches, queue module, and database fit together.
- [Codebase guide](codebase-guide.md) — Understand startup, dependency injection, module structure, and where changes belong.
- [Caching](caching.md) — Understand the local queue-name and shared queue-details caches.

## Run and monitor Retsu

- [Local services](../infra/local/README.md) — Manage PostgreSQL, Dragonfly, and the monitoring tools.
- [Queue state rollups](queue-state-rollups.md) — Understand how queue state is counted without scanning every message.
- [Queue metric cardinality](queue-metric-cardinality.md) — Configure how many queue-labelled time series Retsu preserves.
- [Queue state collector leadership](queue-state-collector-leadership.md) — Keep one collector active while standby replicas provide failover.
- [Local services](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) — Manage the local database and monitoring tools.
