# Benchmarking

Retsu benchmarks compare a candidate commit with the commit where its branch
diverged from `main`. The comparison is intended to answer whether a code
change improves or regresses a named queue operation; it is not an absolute
hardware score.

## Comparison contract

- Run two independent base, candidate, and same-code control measurement pairs
  in the same GitHub Actions job.
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

## Benchmark suite

The suite exercises the public HTTP API and PostgreSQL with an optimized Retsu
binary:

| Measurement | Workloads | Reported value |
| --- | --- | --- |
| Enqueue | 1 KiB payload | Request latency |
| Dequeue | 1 KiB payload at queue depths 1, 1,000, and 10,000 | Request latency |
| Acknowledge | 1 KiB payload | Request latency |
| Full lifecycle | 1 KiB and 64 KiB payloads | Enqueue + dequeue + acknowledge latency |
| Concurrent lifecycle | 4 and 8 workers with 1 KiB payloads | Batch latency and operations per second |

Each operation uses its own queue so one measurement cannot leave state that
changes another measurement. Enqueue cleanup, dequeue restoration, acknowledge
setup, queue creation, migrations, and queue-depth seeding happen outside the
timed interval. Full-lifecycle and concurrent measurements time all three
public requests and finish with an empty queue.

The depth fixtures are inserted directly into PostgreSQL before measurement.
This makes the dequeue result describe the production dequeue path at a known
queue depth instead of timing thousands of setup requests.

GitHub Actions keeps one pinned PostgreSQL service alive for every measurement
pair. Before each base, candidate, or control pass, the harness resets the
dedicated `retsu_benchmark` schema and applies that commit's migrations. This
keeps database process and host conditions constant without carrying queue data
or schema changes between passes.

[Standard Linux runners for private repositories][github-runner-specs] provide
two CPUs. The workflow builds both commits before starting either measurement
pair, so compilation is excluded and cannot warm only the first measured
commit. Retsu, the load generator, and PostgreSQL share the runner under the
operating-system scheduler. The reported values describe a paired comparison
on this runner class, not the absolute capacity of a production host.

Maintenance workers are intentionally outside this suite. Their production
entrypoints are scheduled polling loops, not bounded requests, so a latency
number would mostly describe the configured polling interval. They should be
covered later by a separate sustained-workload benchmark with explicit backlog
and throughput targets.

## Results

The manual workflow writes a Markdown comparison to the GitHub Actions job
summary and retains the complete Criterion report as a workflow artifact.
Concurrent rows include operations per second in addition to batch latency. The
workflow is informational: a reported regression does not fail the job.

GitHub-hosted virtual machines introduce measurement noise. Base and candidate
results therefore remain paired in one job. Each pair measures the base again
after the candidate. A material difference between the initial and control base
measurements marks the combined result as unstable runner drift instead of
attributing that difference to the candidate.

An improvement or regression is reported only when both pairs have stable
controls and agree on the direction. A control is stable only when its entire
change confidence interval fits inside the mode's practical noise band. A
candidate is directional only when its entire change confidence interval lies
outside that band. Stable pairs that disagree or are not directional are
inconclusive. This agreement rule reduces the chance that one unusually fast or
slow hosted-runner interval is mistaken for an application change.

| Mode | Samples | Warm-up per workload | Measurement per workload | Practical noise band |
| --- | ---: | ---: | ---: | ---: |
| Standard | 100 | 5 seconds | 10 seconds | ±10% |
| Thorough | 200 | 10 seconds | 30 seconds | ±5% |

Standard mode is the faster signal for changes larger than about 10% and should
normally complete within 30 minutes when the build cache is warm. Thorough mode
spends more time resolving changes around 5% and has a 60-minute job limit.
Changes below the selected band are deliberately reported as inconclusive;
rerun a result near the boundary before drawing a conclusion.

[github-runner-specs]: https://docs.github.com/en/actions/reference/runners/github-hosted-runners
