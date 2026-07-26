# Getting started

Use this guide to run Retsu on your computer.

Retsu is still being built. You can create queues, but adding and processing
messages is not available yet.

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

3. Start PostgreSQL and prepare the database:

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

You can now [create a queue](queues.md).

Start the worker in a separate terminal:

```bash
just worker
```

The worker starts successfully, but it does not process messages yet.

Run `just` without arguments to see all available commands:

```bash
just
```

## Use the local tools

Start PostgreSQL and all monitoring tools:

```bash
just stack-up
```

Run the API or worker with request tracking enabled:

```bash
just api-observed
just worker-observed
```

See the [local services guide](../infra/local/README.md) for ports, logs, reset
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
