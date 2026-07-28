# Benchmarking

Retsu benchmarks compare a candidate commit with the commit where its branch
diverged from `main`. The comparison is intended to answer whether a code
change improves or regresses a named queue operation; it is not an absolute
hardware score.

## Comparison contract

- Run the base, candidate, and same-code control measurements in the same
  GitHub Actions job.
- Pin the runner image, Rust toolchain, dependency lockfile, and PostgreSQL
  image.
- Exclude compilation, migrations, fixture creation, and warm-up from measured
  time.
- Use identical benchmark definitions and fixtures for both commits.
- Report the exact base and candidate commit identifiers with every result.
- Treat small or statistically inconclusive changes as noise.

The default base is the merge base of the selected candidate branch and
`origin/main`. This isolates changes made on the candidate branch from unrelated
changes that may have landed on `main` later.

## Initial scope

The first benchmark exercises one complete message lifecycle through the public
HTTP API and PostgreSQL:

1. enqueue a message;
2. dequeue that message;
3. acknowledge it.

The lifecycle leaves the queue ready for the next sample and represents a real
user-visible operation. More focused enqueue, dequeue, maintenance, and
concurrency benchmarks can be added after the comparison workflow is proven.

## Results

The manual workflow writes a concise Markdown comparison to the GitHub Actions
job summary and retains the complete Criterion report as a workflow artifact.
The first version is informational: a reported regression does not fail the
workflow.

GitHub-hosted virtual machines introduce measurement noise. Base and candidate
results therefore remain paired in one job. After the candidate, the workflow
measures the base commit again. A material difference between the two base
measurements marks the comparison as unstable runner drift instead of
attributing that difference to the candidate. The report should still be rerun
when an expected improvement is close to the noise threshold.
