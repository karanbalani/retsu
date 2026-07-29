# Getting started

Use this guide to run Retsu on your computer.

Retsu is still being built. You can create and configure queues, add messages, get the next
waiting message, and mark it as complete. Timed-out messages become claimable
again without a background worker.

## What you need

- Rust 1.97.1. The version is set in `rust-toolchain.toml`.
- Docker with Docker Compose.
- Just 1.45 or newer.
- Bash.
- `curl` to check the running application.

## Set up Retsu

1. Install the SQLx command-line tool used by the project:

   ```bash
   just sqlx-install
   ```

2. Check that the required tools are ready:

   ```bash
   just doctor
   ```

3. Start PostgreSQL and Dragonfly, then prepare the database:

   ```bash
   just setup
   ```

## Run Retsu

Start the API:

```bash
just api
```

Leave this command running. In another terminal, check that the API can use the
database:

```bash
curl http://127.0.0.1:2424/health/ready
```

A ready API returns:

```json
{"status":"ready"}
```

Follow the [queues and messages guide](queues.md) to create a queue, add a
message, get the next one, and complete it.

List the available worker module and its workers:

```bash
just worker-modules
just worker-list queue
```

Start the expired message cleaner in another terminal:

```bash
just worker queue expired-message-cleaner
```

The cleaner removes messages after their lifetime ends.

Start a state metrics collector in another terminal. One collector is enough
for local development. Give it another management port:

```bash
RETSU_WORKER__MANAGEMENT__PORT=24252 \
  just worker queue state-metrics-collector
```

It refreshes queue counts and message ages for Prometheus every 15 seconds.
Deployments may run standby collectors for failover; PostgreSQL keeps only one
active. See [queue state collector leadership](queue-state-collector-leadership.md).

Run `just` without arguments to see all available commands:

```bash
just
```

## Use the local tools

Start PostgreSQL and all monitoring tools:

```bash
just stack-up
```

Run the API or queue worker with activity tracking enabled:

```bash
just api-observed
just worker-observed queue expired-message-cleaner
RETSU_WORKER__MANAGEMENT__PORT=24252 \
  just worker-observed queue state-metrics-collector
```

See the [local services guide](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) for ports, logs, reset
commands, and other details.

## Run project checks

Run every local check:

```bash
just quality
```

## Create a database change

Install SQLx first, then create a new migration:

```bash
just migration-new create_queues_and_messages
```

Use a short, lowercase name with words separated by underscores. Do not create
or rename migration files by hand.

Apply pending migrations with `just migrate`.

## Change local settings

Run `just env-init` to create `.env` for Docker ports and passwords. Do not load
that file into Retsu itself.

Application settings are in `config/retsu.yaml`. You can override one setting
for a command:

```bash
RETSU_HTTP__PORT=3000 just api
```
