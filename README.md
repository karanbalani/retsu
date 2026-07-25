# retsu

An observable, distributed priority queue built with Rust, Actix Web, PostgreSQL,
SQLx, OpenTelemetry, and Prometheus.

## Prerequisites

- Rust 1.97.1, installed automatically through `rust-toolchain.toml`
- Docker with Docker Compose
- Just 1.45 or newer
- Bash

## Initial setup

Install the SQLx CLI version used by the project:

```bash
just sqlx-install
```

Verify the development environment:

```bash
just doctor
```

Start PostgreSQL and apply migrations:

```bash
just setup
```

Run the API:

```bash
just api
```

Run background workers:

```bash
just worker
```

Run `just` without arguments to list all available commands:

```bash
just
```

## Complete local stack

Start PostgreSQL and the complete observability stack:

```bash
just stack-up
```

Run the API with trace export enabled:

```bash
just api-observed
```

Run workers with trace export enabled:

```bash
just worker-observed
```

## Quality checks

Run all local quality gates:

```bash
just quality
```

## Database migrations

Create migration files through SQLx:

```bash
just migration-new create_queues_and_messages
```

Migration descriptions must use lowercase `snake_case`. Do not create or rename
migration files manually.

Apply pending migrations:

```bash
just migrate
```

## Local configuration

Create a local Compose configuration file when custom ports or credentials are
required:

```bash
just env-init
```

The root `.env` file is for Docker Compose interpolation only. Do not source it
into Retsu application processes.

Application configuration is loaded from `config/retsu.yaml` and can be
overridden with nested environment variables:

```bash
RETSU_HTTP__PORT=3000 just api
```

See [`infra/local/README.md`](infra/local/README.md) for the complete local
infrastructure guide.
