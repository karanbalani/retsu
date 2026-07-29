# Contributing to Retsu

Thank you for contributing to Retsu.

## Development setup

Retsu requires the Rust toolchain pinned in `rust-toolchain.toml`, Docker with
Docker Compose, Just 1.45 or newer, Bash, and the SQLx CLI.

Install the SQLx CLI and verify the toolchain:

```bash
just sqlx-install
just doctor
```

Start the local database and distributed cache, then apply migrations:

```bash
just setup
```

See the [getting started guide](docs/getting-started.md) for instructions on
running the API, workers, and observability stack.

## Making changes

Create each change from the latest `main` branch:

```bash
git switch main
git pull --ff-only
git switch -c <type>/<short-description>
```

Use a descriptive branch prefix such as `feat/`, `fix/`, `docs/`, `refactor/`,
`test/`, `ci/`, or `chore/`.

Keep changes focused. Add or update tests and documentation when behavior
changes.

For database changes, create forward-only migrations with:

```bash
just migration-new <lowercase_snake_case_name>
```

Do not create, rename, or edit migration filenames by hand.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) with an
imperative, lowercase summary:

```text
feat: add delayed message delivery
fix: prevent duplicate message claims
docs: explain queue visibility timeouts
```

Use `!` and a `BREAKING CHANGE:` footer when a change is not backward
compatible.

## Checks

Run the standard quality gates before opening a pull request:

```bash
just quality
```

For changes that affect the database, cache, containers, or distributed
behavior, also run:

```bash
just quality-full
```

The full check requires Docker because the integration suite uses
Testcontainers.

## Pull requests

All changes must go through a pull request. In the pull request:

- explain what changed and why;
- describe the checks you ran;
- call out breaking changes, migrations, and operational impact;
- keep unrelated work out of the branch; and
- update the branch when GitHub reports it is behind `main`.

The required `Rust quality gates` and `Validate Docker Compose` checks must
pass, CodeQL must not report a blocking security alert, and review
conversations must be resolved before merging.

Add the `run-integration-tests` label when a pull request needs the
Docker-backed integration workflow.

By contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE).
