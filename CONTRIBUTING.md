# Contributing to Retsu

Thank you for contributing to Retsu.

## Development setup

Retsu requires the Rust toolchain pinned in `rust-toolchain.toml`, Docker with Docker Compose, Just 1.45 or newer, Bash, and the SQLx CLI.

Install SQLx and verify the host development tools:

```console
just sqlx-install
just doctor-host
```

Start PostgreSQL and the distributed cache, then apply migrations:

```console
just setup
```

See [Local development](docs/local-development.md) for the container workflow, application commands, workers, monitoring, and project checks.

## Making changes

Create each change from the latest `main`:

```console
git switch main
git pull --ff-only
git switch -c <type>/<short-description>
```

Use a descriptive prefix such as `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `ci/`, or `chore/`.

Keep changes focused. Add or update tests and documentation when behavior changes.

Create forward-only database migrations with:

```console
just migration-new <lowercase_snake_case_name>
```

Do not create, rename, or edit migration filenames by hand.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) with an imperative, lowercase summary:

```text
feat: add delayed message delivery
fix: prevent duplicate message claims
docs: explain queue visibility timeouts
```

Use `!` and a `BREAKING CHANGE:` footer when a change is not backward compatible.

## Checks

Run the standard quality gates before opening a pull request:

```console
just quality
```

For changes that affect the database, cache, containers, or distributed behavior, also run:

```console
just quality-full
```

The full check needs Docker because the integration suite uses Testcontainers.

For documentation changes, install the pinned site dependency and run the strict build:

```console
python -m pip install --requirement requirements-docs.txt
zensical build --clean --strict
```

The documentation build reads the project version from `Cargo.toml` through
`docs_macros.py`; do not duplicate the release version in Markdown.

## Pull requests

All changes must go through a pull request. In the pull request:

- explain what changed and why;
- describe the checks you ran;
- call out breaking changes, migrations, and operational impact;
- keep unrelated work out of the branch;
- update the branch when GitHub reports that it is behind `main`.

The required Rust and local-infrastructure checks must pass, CodeQL must not report a blocking security alert, and review conversations must be resolved before merging.

Add the `run-integration-tests` label when a pull request needs the Docker-backed integration workflow. Later commits do not rerun it automatically; remove and re-add the label for another run.

By contributing, you agree that your contributions are licensed under the [Apache License 2.0](LICENSE).
