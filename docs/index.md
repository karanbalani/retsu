# Documentation

- [Getting started](getting-started.md) — Set up and run Retsu.
- [Architecture](architecture.md) — See how the API, workers, queue module, and database fit together.
- [Codebase guide](codebase-guide.md) — Understand startup, dependency injection, module structure, and where changes belong.
- [Caching](caching.md) — Understand queue-name caching, capacity limits, and the future shared-cache boundary.
- [Queues and messages](queues.md) — Create a queue, add messages, get the next one, and mark it as complete.
- [Queue state rollups](queue-state-rollups.md) — Understand how queue state is counted without scanning every message.
- [Queue metric cardinality](queue-metric-cardinality.md) — Configure how many queue-labelled time series Retsu preserves.
- [Queue state collector leadership](queue-state-collector-leadership.md) — Keep one collector active while standby replicas provide failover.
- [Benchmarking](benchmarking.md) — Compare queue-operation performance between a candidate branch and its base.
- [Local services](../infra/local/README.md) — Manage the local database and monitoring tools.
