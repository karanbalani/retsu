# Workers

Retsu runs each background job as a separate process. Starting one worker does not start the API or another worker.

List the available modules and queue workers:

```console
just worker-modules
just worker-list queue
```

The queue module currently provides three workers.

## Expired-message cleaner

```console
just worker queue expired-message-cleaner
```

This worker permanently removes messages whose lifetime has ended. It processes up to 500 messages by default. After a full batch it waits 50 milliseconds and continues; otherwise it waits 60 seconds.

An expired in-flight message is removed only after its visibility timeout also ends.

## Dead-letter-message cleaner

```console
just worker queue dead-letter-message-cleaner
```

This worker removes retained dead-letter records. The normal default retention period is 14 days. It uses the same default batch size, 50 millisecond full-batch delay, and 60 second normal interval as the expired-message cleaner.

The complete local stack overrides retention to one hour so cleanup can be observed during development.

## State-metrics collector

```console
just worker queue state-metrics-collector
```

This worker refreshes ready, in-flight, and oldest-message measurements every 15 seconds.

Several collector processes can run for failover, but only one performs collection. They compete for a PostgreSQL lock and standby processes retry every 15 seconds. See [State collector failover](queue-state-collector-leadership.md).

## Health and metrics

Each worker serves:

- `GET /health/live`
- `GET /health/ready`
- `GET /metrics`

The default management address is `127.0.0.1:24247`. Processes on the same computer need different ports:

```console
just worker queue expired-message-cleaner

RETSU_WORKER__MANAGEMENT__PORT=24253 \
  just worker queue dead-letter-message-cleaner

RETSU_WORKER__MANAGEMENT__PORT=24252 \
  just worker queue state-metrics-collector
```

The container-based local stack assigns these ports automatically.

## Shutdown

Workers stop accepting new work when they receive a shutdown signal and allow the current task to finish within `worker.shutdown_timeout_seconds`. The default is 30 seconds.

Configuration and accepted ranges are listed in [Configuration](configuration.md). Worker metrics and traces are covered in [Monitoring](observability.md).
