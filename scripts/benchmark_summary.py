#!/usr/bin/env python3

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass(frozen=True)
class BenchmarkResult:
    name: str
    base_nanoseconds: float
    candidate_nanoseconds: float
    throughput_elements: Optional[int]
    change: float
    change_lower_bound: float
    change_upper_bound: float


@dataclass(frozen=True)
class BenchmarkPair:
    name: str
    results: list[BenchmarkResult]
    controls: list[BenchmarkResult]


def read_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as source:
        return json.load(source)


def load_results(criterion_directory: Path, baseline: str) -> list[BenchmarkResult]:
    results = []

    for change_path in sorted(criterion_directory.glob("**/change/estimates.json")):
        benchmark_directory = change_path.parent.parent
        base_path = benchmark_directory / baseline / "estimates.json"
        candidate_path = benchmark_directory / "new" / "estimates.json"
        metadata_path = benchmark_directory / "new" / "benchmark.json"

        if not base_path.exists() or not candidate_path.exists() or not metadata_path.exists():
            continue

        base = read_json(base_path)["mean"]
        candidate = read_json(candidate_path)["mean"]
        change = read_json(change_path)["mean"]
        metadata = read_json(metadata_path)
        throughput = metadata.get("throughput")
        throughput_elements = (
            throughput.get("Elements") if isinstance(throughput, dict) else None
        )

        results.append(
            BenchmarkResult(
                name=metadata["full_id"],
                base_nanoseconds=base["point_estimate"],
                candidate_nanoseconds=candidate["point_estimate"],
                throughput_elements=throughput_elements,
                change=change["point_estimate"],
                change_lower_bound=change["confidence_interval"]["lower_bound"],
                change_upper_bound=change["confidence_interval"]["upper_bound"],
            )
        )

    if not results:
        raise ValueError(f"no Criterion comparisons found in {criterion_directory}")

    return results


def format_duration(nanoseconds: float) -> str:
    if nanoseconds < 1_000:
        return f"{nanoseconds:.2f} ns"
    if nanoseconds < 1_000_000:
        return f"{nanoseconds / 1_000:.2f} µs"
    if nanoseconds < 1_000_000_000:
        return f"{nanoseconds / 1_000_000:.2f} ms"
    return f"{nanoseconds / 1_000_000_000:.2f} s"


def format_percentage(value: float) -> str:
    return f"{value:+.2%}".replace("-", "−")


def format_measurement(nanoseconds: float, throughput_elements: Optional[int]) -> str:
    duration = format_duration(nanoseconds)

    if throughput_elements is None:
        return duration

    operations_per_second = throughput_elements * 1_000_000_000 / nanoseconds
    return f"{duration} ({operations_per_second:,.0f} ops/s)"


def verdict(result: BenchmarkResult, noise_threshold: float) -> str:
    if result.change_upper_bound < -noise_threshold:
        return "🟢 Improved"
    if result.change_lower_bound > noise_threshold:
        return "🔴 Regressed"
    return "⚪ Inconclusive"


def control_is_stable(control: BenchmarkResult, noise_threshold: float) -> bool:
    return (
        control.change_lower_bound >= -noise_threshold
        and control.change_upper_bound <= noise_threshold
    )


def pair_verdict(
    result: BenchmarkResult,
    control: BenchmarkResult,
    noise_threshold: float,
) -> str:
    if not control_is_stable(control, noise_threshold):
        return "🟠 Unstable"

    return verdict(result, noise_threshold)


def combined_verdict(pair_verdicts: list[str]) -> str:
    if any(pair_result == "🟠 Unstable" for pair_result in pair_verdicts):
        return "🟠 Unstable"
    if all(pair_result == "🟢 Improved" for pair_result in pair_verdicts):
        return "🟢 Improved"
    if all(pair_result == "🔴 Regressed" for pair_result in pair_verdicts):
        return "🔴 Regressed"
    return "⚪ Inconclusive"


def format_change(result: BenchmarkResult) -> str:
    return (
        f"{format_percentage(result.change)} "
        f"({format_percentage(result.change_lower_bound)} to "
        f"{format_percentage(result.change_upper_bound)})"
    )


def format_pair_cell(
    result: BenchmarkResult,
    control: BenchmarkResult,
    result_verdict: str,
) -> str:
    base = format_measurement(result.base_nanoseconds, result.throughput_elements)
    candidate = format_measurement(
        result.candidate_nanoseconds,
        result.throughput_elements,
    )

    return (
        f"{base} → {candidate}<br>"
        f"{format_change(result)}<br>"
        f"same-code control {format_change(control)}<br>"
        f"{result_verdict}"
    )


def format_count(count: int, noun: str) -> str:
    suffix = "" if count == 1 else "s"
    return f"{count} {noun}{suffix}"


def render_conclusion(outcome_counts: dict[str, int], noise_threshold: float) -> str:
    improved = outcome_counts["🟢 Improved"]
    regressed = outcome_counts["🔴 Regressed"]

    findings = []
    if improved:
        findings.append(f"{format_count(improved, 'workload')} improved")
    if regressed:
        findings.append(f"{format_count(regressed, 'workload')} regressed")

    if not findings:
        return (
            "**This run did not establish a repeatable improvement or regression "
            f"beyond the ±{noise_threshold:.0%} practical noise band.**"
        )

    return (
        "**This run established repeatable evidence that "
        + " and ".join(findings)
        + f", beyond the ±{noise_threshold:.0%} practical noise band.**"
    )


def render_markdown(
    pairs: list[BenchmarkPair],
    base: str,
    candidate: str,
    mode: str,
    noise_threshold: float,
) -> str:
    if not pairs:
        raise ValueError("at least one benchmark pair is required")

    benchmark_names = [result.name for result in pairs[0].results]
    expected_names = set(benchmark_names)
    pair_results = []

    for pair in pairs:
        results_by_name = {result.name: result for result in pair.results}
        controls_by_name = {control.name: control for control in pair.controls}

        if set(results_by_name) != expected_names:
            raise ValueError(f"{pair.name} has a different benchmark result set")
        if set(controls_by_name) != expected_names:
            raise ValueError(f"{pair.name} has a different same-code control set")

        pair_results.append((pair.name, results_by_name, controls_by_name))

    rendered_rows = []

    for benchmark_name in benchmark_names:
        cells = []
        result_verdicts = []

        for _, results_by_name, controls_by_name in pair_results:
            result = results_by_name[benchmark_name]
            control = controls_by_name[benchmark_name]
            result_verdict = pair_verdict(result, control, noise_threshold)
            result_verdicts.append(result_verdict)
            cells.append(format_pair_cell(result, control, result_verdict))

        rendered_rows.append(
            (benchmark_name, cells, combined_verdict(result_verdicts))
        )

    outcomes = ("🟢 Improved", "🔴 Regressed", "⚪ Inconclusive", "🟠 Unstable")
    outcome_counts = {
        outcome: sum(result_verdict == outcome for _, _, result_verdict in rendered_rows)
        for outcome in outcomes
    }

    lines = [
        "## Performance benchmark",
        "",
        "### What this run is trying to establish",
        "",
        (
            "This report asks whether the candidate commit makes the tested queue "
            "operations consistently faster or slower than the point where its branch "
            "diverged from `main`."
        ),
        "",
        (
            "Both commits run the same workloads as optimized release binaries against "
            "the same PostgreSQL service on one GitHub-hosted runner. This is a relative "
            "comparison between two commits, not an absolute production-capacity test."
        ),
        "",
        "### Result at a glance",
        "",
        render_conclusion(outcome_counts, noise_threshold),
        "",
        (
            f"- Outcomes: {outcome_counts['🟢 Improved']} improved · "
            f"{outcome_counts['🔴 Regressed']} regressed · "
            f"{outcome_counts['⚪ Inconclusive']} inconclusive · "
            f"{outcome_counts['🟠 Unstable']} unstable"
        ),
        (
            f"- Inconclusive means the runner was stable, but the change could not be "
            f"distinguished from the ±{noise_threshold:.0%} noise band in both pairs."
        ),
        (
            "- Unstable means unchanged base code also moved materially, so runner drift "
            "prevents attributing the result to the candidate."
        ),
        "",
        "### How the comparison works",
        "",
        (
            f"Two independent rounds each run **base → candidate → base control**. "
            f"The repeated base is the same code as the first base and detects changes "
            f"in runner conditions. A workload is called improved or regressed only "
            f"when both controls are stable and both candidate comparisons agree."
        ),
        "",
        "Lower duration is better. Concurrent workloads also show operations per second, "
        "where higher is better. The percentage in parentheses is Criterion's confidence "
        "interval for the measured change.",
        "",
        "### Workload results",
        "",
    ]

    pair_headers = [
        pair_name.replace("|", "\\|") + " (base → candidate)"
        for pair_name, _, _ in pair_results
    ]
    lines.append("| Benchmark | " + " | ".join(pair_headers) + " | Verdict |")
    lines.append("| --- | " + " | ".join("---:" for _ in pairs) + " | --- |")

    for benchmark_name, cells, result_verdict in rendered_rows:
        name = benchmark_name.replace("|", "\\|")
        lines.append(
            f"| `{name}` | " + " | ".join(cells) + f" | {result_verdict} |"
        )

    lines.extend(
        [
            "",
            "### Verdict guide",
            "",
            "| Verdict | What it means |",
            "| --- | --- |",
            (
                "| 🟢 Improved | Both measurement pairs show a speed-up beyond the "
                "noise band, with stable same-code controls. |"
            ),
            (
                "| 🔴 Regressed | Both measurement pairs show a slow-down beyond the "
                "noise band, with stable same-code controls. |"
            ),
            (
                "| ⚪ Inconclusive | Runner conditions were stable, but the change was "
                "inside the noise band or the two pairs did not agree. |"
            ),
            (
                "| 🟠 Unstable | Unchanged base code drifted beyond the noise band, so "
                "no performance claim is made. |"
            ),
            "",
            "### Run details",
            "",
            f"- Base commit: `{base}`",
            f"- Candidate commit: `{candidate}`",
            f"- Measurement mode: `{mode}`",
            f"- Practical noise band: `±{noise_threshold:.0%}`",
            f"- Comparison pairs: `{len(pairs)}`",
            "- Optimized release binaries: `yes`",
            "",
            "> Informational only: the benchmark reports evidence but does not fail the "
            "workflow on a regression. Rerun borderline or unstable results before "
            "making a release decision.",
        ]
    )

    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description="Render Criterion comparisons as Markdown")
    parser.add_argument("--criterion-directory", type=Path, default=Path("target/criterion"))
    parser.add_argument("--control-directory", type=Path)
    parser.add_argument("--baseline", default="base")
    parser.add_argument(
        "--pair",
        action="append",
        nargs=4,
        metavar=("NAME", "CANDIDATE_DIRECTORY", "CONTROL_DIRECTORY", "BASELINE"),
    )
    parser.add_argument("--base", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--mode", required=True)
    parser.add_argument("--noise-threshold", type=float, default=0.01)
    arguments = parser.parse_args()

    if arguments.pair:
        pairs = [
            BenchmarkPair(
                name=name,
                results=load_results(Path(candidate_directory), baseline),
                controls=load_results(Path(control_directory), baseline),
            )
            for name, candidate_directory, control_directory, baseline in arguments.pair
        ]
    else:
        if arguments.control_directory is None:
            parser.error("--control-directory is required when --pair is not provided")

        pairs = [
            BenchmarkPair(
                name="Pair 1",
                results=load_results(
                    arguments.criterion_directory,
                    arguments.baseline,
                ),
                controls=load_results(
                    arguments.control_directory,
                    arguments.baseline,
                ),
            )
        ]

    print(
        render_markdown(
            pairs,
            arguments.base,
            arguments.candidate,
            arguments.mode,
            arguments.noise_threshold,
        )
    )


if __name__ == "__main__":
    main()
