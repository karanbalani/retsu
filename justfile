set shell := ["bash", "-euo", "pipefail", "-c"]

sqlx-cli-version := "0.9.0"
observability-services := "otel-collector tempo prometheus pg-exporter cadvisor grafana"

# Docker Compose reads .env itself. Do not export Compose-only
# RETSU_LOCAL_* variables into application processes.
set dotenv-load := false

[private]
default:
    @just --list

# Show available development commands.
help:
    @just --list

# Check justfile formatting.
justfile-check:
    just --fmt --check

# Verify the local development toolchain.
doctor:
    rustc --version
    cargo --version
    just --version
    docker --version
    docker compose version
    @command -v sqlx >/dev/null || { \
        echo "SQLx CLI is missing. Run: just sqlx-install"; \
        exit 1; \
    }
    sqlx --version
    @docker info >/dev/null
    @echo "Development environment is ready."

# Create .env without overwriting existing local configuration.
env-init:
    @if [ -e .env ]; then \
        echo ".env already exists; left unchanged."; \
    else \
        cp .env.example .env; \
        echo "Created .env from .env.example."; \
    fi

# Build all targets and features.
build:
    cargo build --locked --all-targets --all-features

# Build the optimized production binary.
release-build:
    cargo build --locked --release --bin retsu

# Create and push an annotated calendar-version release tag.
[arg('version', pattern='[1-9][0-9]{3}\.(?:[1-9]|1[0-2])\.(?:0|[1-9][0-9]*)')]
release-tag version:
    ./scripts/release-tag.sh "{{ version }}"

# Type-check all targets and features.
check:
    cargo check --locked --all-targets --all-features

# Format Rust sources.
fmt:
    cargo fmt --all

# Check source formatting.
fmt-check:
    cargo fmt --all -- --check

# Run Clippy and reject warnings.
lint:
    cargo clippy --locked --all-targets --all-features -- -D warnings

# Run fast unit tests without external services.
test:
    cargo test --locked --lib --all-features

# Run black-box integration tests with ephemeral Testcontainers dependencies.
integration-test:
    cargo test --locked --test integration --all-features -- --test-threads=1

# Validate Docker Compose.
compose-check:
    docker compose config --quiet

# Run all local quality gates.
quality: justfile-check fmt-check lint test compose-check

# Run local quality gates followed by the Docker-backed integration suite.
quality-full: quality integration-test

# Reapply the idempotent PostgreSQL observability bootstrap.
db-observability-init:
    docker compose exec -T postgres \
        sh /docker-entrypoint-initdb.d/observability.sh

# Start PostgreSQL and the distributed cache, then wait until healthy.
db-up:
    docker compose up -d --wait postgres dragonfly
    just db-observability-init

# Stop PostgreSQL and the distributed cache without deleting PostgreSQL data.
db-stop:
    docker compose stop postgres dragonfly

# Open psql inside the PostgreSQL container.
db-shell:
    docker compose exec postgres sh -c 'exec psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB"'

# Stop observability services without deleting their data.
observability-stop:
    docker compose stop {{ observability-services }}

# Start PostgreSQL and the complete observability stack.
stack-up: db-up
    docker compose --profile observability up -d --wait

# Show every local service, including stopped services.
stack-status:
    docker compose --profile observability ps --all

# Follow logs for a Compose service.
logs service="postgres":
    docker compose --profile observability logs --follow "{{ service }}"

# Stop and remove all local containers while preserving data.
stack-down:
    docker compose --profile observability down --remove-orphans

[private]
_stack-wipe:
    docker compose --profile observability down --volumes --remove-orphans

# Delete all PostgreSQL, Prometheus, Tempo, and Grafana data.
[confirm("Delete the complete local stack and all persisted data?")]
stack-wipe: _stack-wipe

# Recreate the complete stack from scratch and apply migrations.
[confirm("Delete and recreate PostgreSQL, Prometheus, Tempo, and Grafana from scratch?")]
stack-reset config="config/retsu.yaml": _stack-wipe
    docker compose --profile observability up -d --wait
    cargo run --locked -- --config "{{ config }}" migrate

# Apply application-owned migrations.
migrate config="config/retsu.yaml":
    cargo run --locked -- --config "{{ config }}" migrate

# Run the API.
api config="config/retsu.yaml":
    cargo run --locked -- --config "{{ config }}" api

# Run the API with trace export enabled.
api-observed config="config/retsu.yaml":
    RETSU_TELEMETRY__TRACES__ENABLED=true cargo run --locked -- --config "{{ config }}" api

# Run one named background worker.
worker module name config="config/retsu.yaml":
    cargo run --locked -- \
        --config "{{ config }}" \
        worker run "{{ module }}" "{{ name }}"

# Run one named background worker with trace export enabled.
worker-observed module name config="config/retsu.yaml":
    RETSU_TELEMETRY__TRACES__ENABLED=true \
        cargo run --locked -- \
        --config "{{ config }}" \
        worker run "{{ module }}" "{{ name }}"

# List modules that contribute workers.
worker-modules:
    cargo run --locked -- worker list

# List workers contributed by one module.
worker-list module:
    cargo run --locked -- worker list "{{ module }}"

# Install the SQLx CLI version matching the project.
sqlx-install:
    cargo install sqlx-cli \
        --version {{ sqlx-cli-version }} \
        --no-default-features \
        --features rustls,postgres

# Create a forward-only timestamped migration.
#
# Names must use lowercase snake_case:
# just migration-new create_queues_and_messages
[arg('name', pattern='[a-z][a-z0-9]*(?:_[a-z0-9]+)*')]
migration-new name:
    @command -v sqlx >/dev/null || { \
        echo "SQLx CLI is missing. Run: just sqlx-install"; \
        exit 127; \
    }
    sqlx migrate add --source migrations "{{ name }}"

# Start PostgreSQL and apply migrations.
setup: db-up migrate
