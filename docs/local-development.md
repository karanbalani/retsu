# Local development

Use the container workflow to run the complete product. Use the host workflow when changing Rust code.

## Container workflow

Check the container tools and start everything:

```console
just doctor
just local-up
```

The stack builds the same hardened image used for production and starts migrations, the API, all workers, PostgreSQL, the distributed cache, and monitoring.

Useful commands:

| Command | Purpose |
| --- | --- |
| `just local-ready` | Check the API, workers, Prometheus, and Grafana |
| `just local-status` | Show every local service |
| `just local-stop` | Stop services and keep data |
| `just stack-down` | Remove containers and keep data |
| `just logs api` | Follow one service's logs |
| `just local-validate` | Check local configuration without starting it |

See the [detailed local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) for every service, port, resource limit, and data command.

## Host Rust workflow

The project uses the Rust version in `rust-toolchain.toml`.

Install the pinned SQLx command-line tool and check the host toolchain:

```console
just sqlx-install
just doctor-host
```

Start PostgreSQL and Dragonfly, then apply migrations:

```console
just setup
```

Run the API:

```console
just api
```

Run one worker in another terminal:

```console
just worker queue expired-message-cleaner
```

See [Workers](workers.md) for all worker commands and management ports.

To send traces from a host process to the local monitoring stack, use `just stack-up` first, then run `just api-observed` or `just worker-observed`.

## Project checks

Run formatting checks, Clippy, unit tests, and local Compose validation:

```console
just quality
```

Run the black-box integration suite:

```console
just integration-test
```

Run both:

```console
just quality-full
```

The integration suite needs Docker but not the local Compose stack. It starts isolated PostgreSQL and Dragonfly containers, applies migrations, and exercises the compiled Retsu processes through HTTP.

## Documentation checks

Install the pinned documentation dependency and build the site:

```console
python -m pip install --requirement requirements-docs.txt
zensical build --clean --strict
```

The documentation workflow runs the same strict build for pull requests that change the site.
It reads the project version from `Cargo.toml`, so release-version changes
automatically rebuild the versioned examples.

## Database migrations

Create a forward-only migration:

```console
just migration-new create_queues_and_messages
```

Use a short lowercase name with underscores. Do not create or rename migration files by hand.

Apply pending migrations with `just migrate`.

## Local settings

`config/retsu.yaml` contains application settings. Override one value with a `RETSU_` environment variable:

```console
RETSU_HTTP__PORT=3000 just api
```

Run `just env-init` to create the root `.env` used by Docker Compose. Its `RETSU_LOCAL_*` variables are Compose settings, not application settings. Do not source that file into a host Retsu process.

See [Configuration](configuration.md) for every application setting.
