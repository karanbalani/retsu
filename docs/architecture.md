# Architecture

This page shows how the current parts of Retsu fit together. It describes the code on `main`, not planned features.

## Running parts

Retsu is one program that can run as an API, one named worker, or a database migration. The API and each worker run as separate processes. They load the same configuration, create their own database connections, and use the queue module.

```mermaid
flowchart LR
    Client["Application using Retsu"] --> API["API process<br/>HTTP routes + queue module"]
    API --> Memory["In-memory queue names"]
    Memory --> Distributed["Distributed queue details"]
    Distributed --> Database[("PostgreSQL")]
    Cleaner["Worker process<br/>expired-message-cleaner<br/>+ queue module"] --> Database
    State["Worker process<br/>state-metrics-collector<br/>+ queue module"] --> Database
    API --> Database
    Prometheus["Prometheus"] -. "reads metrics" .-> API
    Prometheus -. "reads metrics" .-> Cleaner
    Prometheus -. "reads metrics" .-> State
    API -. "sends traces when enabled" .-> Collector["Trace collector"]
    Cleaner -. "sends traces when enabled" .-> Collector
    State -. "sends traces when enabled" .-> Collector
```

The queue module is part of each process, not a separate service. It holds the
queue rules, queue metadata caches, and PostgreSQL operations used by the API
and workers. PostgreSQL remains authoritative. Queue names are cached in each
process; complete queue details are cached in the distributed Redis-protocol
store.

The API receives health and queue requests. A dequeue request can directly
claim a returned message whose visibility timeout has ended and moves exhausted
messages to dead-letter storage as bounded maintenance. A worker process runs
one selected background job. The expired message cleaner removes messages whose
lifetime has ended. State metrics collector replicas compete for a PostgreSQL
leadership lock. One active collector refreshes queue counts and message ages
every 15 seconds while the others wait to take over. Each worker process also
serves its own health and metrics endpoints.

Prometheus reads measurements from the API and workers. When trace export is enabled, they send activity details to the trace collector.

Starting one queue worker does not start the API or any other worker.

## Start or inspect a process

| Command | What it does |
| --- | --- |
| `just api` | Starts the HTTP API |
| `just worker-modules` | Lists modules that provide workers |
| `just worker-list queue` | Lists workers provided by the queue module |
| `just worker queue expired-message-cleaner` | Starts the expired message cleaner |
| `just worker queue state-metrics-collector` | Starts the queue state metrics collector |
| `just migrate` | Applies pending database changes |

An application module groups one feature's API routes, rules, database work, and workers. The queue module is the only application module today.

Each worker starts a health and metrics server on the configured management port. Give workers different ports when running more than one on the same computer.

## Message lifecycle

```mermaid
flowchart TD
    Added["Message added"] --> Waiting["Waiting in the queue"]
    Waiting --> Delivered["Returned to an API client"]
    Waiting -->|"Lifetime ends"| Expired["Removed by<br/>expired-message-cleaner"]
    Delivered --> Completed{"Completed before its timeout?"}
    Delivered -->|"Lifetime and visibility timeout end"| Expired
    Completed -->|"Yes"| Removed["Removed"]
    Completed -->|"No"| Limit{"Delivery attempt limit reached?"}
    Limit -->|"No"| Waiting
    Limit -->|"Yes"| Stored["Removed from the active queue<br/>and stored separately"]
```

Returning a message creates a receipt handle, advances its `available_after`
timestamp, and increases its delivery attempt count. Completing it with the
current unexpired receipt handle removes it. If its visibility timeout ends
first, a later dequeue can claim it directly or store it separately when the
attempt limit has been reached.

The expired message cleaner removes an expired waiting message. If a returned message expires, the cleaner waits for its visibility timeout to end before removing it.

See [Queues and messages](queues.md) for the requests, responses, and settings
used in this flow. See [Caching](caching.md) for the in-memory and distributed
queue metadata paths. See the [Codebase guide](codebase-guide.md) to
understand the dependency setup and module boundaries. See
[Local services](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) for the database, distributed cache,
and monitoring tools. The queue metrics guides explain
[state rollups](queue-state-rollups.md),
[metric cardinality](queue-metric-cardinality.md), and
[collector leadership](queue-state-collector-leadership.md).

## Where the code lives

| Path | What it contains |
| --- | --- |
| `src/entrypoints/` | Starts the API, a selected worker, or migrations |
| `src/cache/` | In-memory and Redis-protocol cache implementations |
| `src/modules/` | Registers each application module's routes and workers |
| `src/modules/queue/` | Queue rules, API handlers, persistence, caching, and queue workers |
| `src/worker/` | Runs the selected worker and its health and metrics server |
| `migrations/` | PostgreSQL database changes |
