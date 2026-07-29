# Deployment and releases

Retsu publishes one production image that can run migrations, the API, or one named worker.

## Image

Published images use:

```text
ghcr.io/karanbalani/retsu:YEAR.MONTH.RELEASE
```

For example:

```console
docker pull ghcr.io/karanbalani/retsu:{{ retsu_version }}
docker run --rm ghcr.io/karanbalani/retsu:{{ retsu_version }} --version
```

Every release also has a `sha-<commit>` tag. There is no `latest` tag, so deployments must choose an explicit version or commit.

Images are built for Linux AMD64 and ARM64. The runtime image:

- runs as user and group `65532`;
- contains the Retsu binary, required system libraries, and certificate authorities;
- has no shell or package manager;
- exposes API port `2424` and worker management port `24247`;
- uses `api` as its default command.

## Runtime roles

Use the same image for each role:

```console
docker run --rm [settings] ghcr.io/karanbalani/retsu:{{ retsu_version }} migrate
docker run --rm [settings] ghcr.io/karanbalani/retsu:{{ retsu_version }} api
docker run --rm [settings] ghcr.io/karanbalani/retsu:{{ retsu_version }} worker run queue expired-message-cleaner
docker run --rm [settings] ghcr.io/karanbalani/retsu:{{ retsu_version }} worker run queue dead-letter-message-cleaner
docker run --rm [settings] ghcr.io/karanbalani/retsu:{{ retsu_version }} worker run queue state-metrics-collector
```

Replace `[settings]` with environment variables, network settings, and port mappings for the deployment. At minimum, every role needs the PostgreSQL URL. Queue operations also need the distributed cache URL when that cache is enabled.

Example API settings:

```console
docker run --rm \
  --publish 2424:2424 \
  --env RETSU_HTTP__BIND_ADDRESS=0.0.0.0 \
  --env RETSU_DATABASE__URL=postgres://user:password@database:5432/retsu \
  --env RETSU_CACHE__DISTRIBUTED__URL=redis://cache:6379 \
  ghcr.io/karanbalani/retsu:{{ retsu_version }} api
```

Use a secret manager instead of putting production credentials directly in a shell command.

## Rollout order

1. Run `migrate` as a one-time job.
2. Start or update the API.
3. Start each required worker as its own process.
4. Check `/health/ready` before sending traffic or marking a worker ready.
5. Scrape `/metrics` from the API and every worker.

Run migrations once per release rollout. The API and workers do not apply them automatically.

Each process has its own database pool. Each state collector leader also holds one dedicated database connection for its PostgreSQL lock.

## Configuration

The image does not contain the repository's local YAML file. Provide `RETSU_` environment variables or mount a YAML file and pass it explicitly:

```console
retsu --config /etc/retsu.yaml api
```

See [Configuration](configuration.md) and [Workers](workers.md).

## Creating a release

Maintainers create calendar-version tags from a clean local `main` that exactly matches `origin/main`:

```console
just release-tag {{ retsu_version }}
```

The command asks for confirmation and pushes the annotated `v{{ retsu_version }}` tag.

The release workflow then:

1. checks the tag format and confirms its commit belongs to `main`;
2. builds and publishes the AMD64 and ARM64 images;
3. attaches build provenance and a software bill of materials;
4. publishes the version and commit tags without `latest`;
5. creates a GitHub release with generated notes.
