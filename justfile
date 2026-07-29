set shell := ["bash", "-euo", "pipefail", "-c"]

sqlx-cli-version := "0.9.0"
observability-services := "prometheus pg-exporter cadvisor grafana"
tracing-services := "otel-collector tempo"
compose := "docker compose --file infra/local/compose.yaml"

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
    {{ compose }} --profile observability --profile tracing --profile load config --quiet

# Build the hardened image used by every local Retsu role.
local-build:
    {{ compose }} build migrate

# Verify the local API, workers, Prometheus, and Grafana.
local-ready:
    ./scripts/local/ready.sh

# Build and start the complete local runtime and observability stack.
local-up: local-build
    ./scripts/local/up.sh

# Start the optional local tracing services after the local stack.
local-up-tracing: local-up
    ./scripts/local/up-tracing.sh

# Show all local services, including stopped services.
local-status:
    {{ compose }} --profile observability --profile tracing --profile load ps --all

# Stop the complete local stack without deleting persisted data.
local-stop:
    {{ compose }} --profile observability --profile tracing --profile load stop

# Run one disposable local load scenario.
local-load scenario="smoke":
    ./scripts/local/load.sh "{{ scenario }}"

# Run all local quality gates.
quality: justfile-check fmt-check lint test compose-check

# Run local quality gates followed by the Docker-backed integration suite.
quality-full: quality integration-test

# Reapply the idempotent PostgreSQL observability bootstrap.
db-observability-init:
    {{ compose }} exec -T postgres \
        sh /docker-entrypoint-initdb.d/observability.sh

# Start PostgreSQL and the distributed cache, then wait until healthy.
db-up:
    {{ compose }} up -d --wait postgres dragonfly
    just db-observability-init

# Stop PostgreSQL and the distributed cache without deleting PostgreSQL data.
db-stop:
    {{ compose }} stop postgres dragonfly

# Open psql inside the PostgreSQL container.
db-shell:
    {{ compose }} exec postgres sh -c 'exec psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB"'

# Stop observability services without deleting their data.
observability-stop:
    {{ compose }} --profile observability --profile tracing --profile load stop {{ observability-services }} {{ tracing-services }}

# Start PostgreSQL and the complete observability stack.
stack-up: db-up
    {{ compose }} --profile observability --profile tracing up -d --wait {{ observability-services }} {{ tracing-services }}

# Show every local service, including stopped services.
stack-status:
    {{ compose }} --profile observability --profile tracing --profile load ps --all

# Follow logs for a Compose service.
logs service="postgres":
    {{ compose }} --profile observability --profile tracing --profile load logs --follow "{{ service }}"

# Stop and remove all local containers while preserving data.
stack-down:
    {{ compose }} --profile observability --profile tracing --profile load down --remove-orphans

[private]
_stack-wipe:
    {{ compose }} --profile observability --profile tracing --profile load down --volumes --remove-orphans

# Delete all PostgreSQL, Prometheus, Tempo, and Grafana data.
[confirm("Delete the complete local stack and all persisted data?")]
stack-wipe: _stack-wipe

# Recreate the complete stack from scratch and apply migrations.
[confirm("Delete and recreate PostgreSQL, Prometheus, Tempo, and Grafana from scratch?")]
stack-reset config="config/retsu.yaml": _stack-wipe
    {{ compose }} up -d --wait postgres dragonfly
    {{ compose }} --profile observability --profile tracing up -d --wait {{ observability-services }} {{ tracing-services }}
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
