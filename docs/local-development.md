# Local development

Use containers to run the complete product. Use the host Rust toolchain when changing code.

## Run the complete stack

```console
just doctor
just local-up
```

Useful commands:

| Command | Purpose |
| --- | --- |
| `just local-ready` | Check the API, workers, Prometheus, and Grafana |
| `just local-status` | Show every local service |
| `just logs api` | Follow the API logs |
| `just local-stop` | Stop services and keep data |
| `just stack-down` | Remove containers and keep data |

The [local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) has the full list of services, ports, and data-management commands.

## Run Rust on the host

Install the pinned SQLx tool, check the toolchain, and start PostgreSQL and Dragonfly:

```console
just sqlx-install
just doctor-host
just setup
```

Run the API:

```console
just api
```

Run a worker in another terminal:

```console
just worker queue expired-message-cleaner
```

Use `just worker-list queue` to see every queue worker. Start the monitoring stack with `just stack-up` before using `just api-observed` or `just worker-observed`.

## Run the checks

```console
just quality
just integration-test
```

`just quality` runs formatting checks, Clippy, unit tests, and Compose validation. The integration suite starts isolated PostgreSQL and Dragonfly containers and exercises real Retsu processes through HTTP.

Run both with:

```console
just quality-full
```

## Create a migration

```console
just migration-new create_queues_and_messages
just migrate
```

Use a short lowercase migration name with underscores. Do not create or rename migration files by hand.

## Run load tests

Only test an environment you own. Start the local stack, then choose a k6 scenario:

```console
just local-load smoke
just local-load enqueue
just local-load consume
just local-load mixed
just local-load saturation
```

| Scenario | Purpose |
| --- | --- |
| `smoke` | Check one complete message lifecycle |
| `enqueue` | Measure writes while building a backlog |
| `consume` | Measure dequeue and acknowledgement throughput |
| `mixed` | Run producers and consumers together |
| `saturation` | Increase demand until the system stops recovering |
| `showcase` | Demonstrate priorities, retries, expiry, and dead letters |

Open the [showcase dashboard](http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase) and run `just local-showcase` for the five-minute demonstration.

Scenario settings can be passed as environment variables:

```console
RUN_ID=regression-001 \
MIXED_PRODUCER_RATE=25 \
MIXED_CONSUMER_RATE=25 \
  just local-load mixed
```

The available settings are defined in `load/k6/support/config.js`. Load runs leave their queues in PostgreSQL, so use a disposable database for repeated performance work.

## Build the documentation

```console
python -m pip install --requirement requirements-docs.txt
zensical build --clean --strict
```
