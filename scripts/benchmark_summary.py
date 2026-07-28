#!/usr/bin/env python3

import argparse
import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class BenchmarkResult:
    name: str
    base_nanoseconds: float
    candidate_nanoseconds: float
    change: float
    change_lower_bound: float
    change_upper_bound: float


def read_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as source:
        return json.load(source)


def load_results(criterion_directory: Path, baseline: str) -> list[BenchmarkResult]:
    results = []

    for change_path in sorted(criterion_directory.glob("*/change/estimates.json")):
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

        results.append(
            BenchmarkResult(
                name=metadata["full_id"],
                base_nanoseconds=base["point_estimate"],
                candidate_nanoseconds=candidate["point_estimate"],
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


def verdict(result: BenchmarkResult, noise_threshold: float) -> str:
    if result.change_upper_bound < -noise_threshold:
        return "🟢 Improved"
    if result.change_lower_bound > noise_threshold:
        return "🔴 Regressed"
    return "⚪ Inconclusive"


def control_is_stable(control: BenchmarkResult, noise_threshold: float) -> bool:
    return (
        control.change_lower_bound <= noise_threshold
        and control.change_upper_bound >= -noise_threshold
    )


def render_markdown(
    results: list[BenchmarkResult],
    controls: list[BenchmarkResult],
    base: str,
    candidate: str,
    mode: str,
    noise_threshold: float,
) -> str:
    controls_by_name = {control.name: control for control in controls}

    lines = [
        "## Benchmark comparison",
        "",
        f"- Base: `{base}`",
        f"- Candidate: `{candidate}`",
        f"- Mode: `{mode}`",
        f"- Practical noise threshold: `{noise_threshold:.0%}`",
        "",
        "| Benchmark | Base mean | Candidate mean | Candidate change (95% CI) | Same-code control | Verdict |",
        "| --- | ---: | ---: | ---: | ---: | --- |",
    ]

    for result in results:
        control = controls_by_name.get(result.name)
        if control is None:
            raise ValueError(f"no same-code control found for {result.name}")

        change = (
            f"{format_percentage(result.change)} "
            f"({format_percentage(result.change_lower_bound)} to "
            f"{format_percentage(result.change_upper_bound)})"
        )
        control_change = (
            f"{format_percentage(control.change)} "
            f"({format_percentage(control.change_lower_bound)} to "
            f"{format_percentage(control.change_upper_bound)})"
        )
        result_verdict = (
            verdict(result, noise_threshold)
            if control_is_stable(control, noise_threshold)
            else "🟠 Unstable"
        )
        name = result.name.replace("|", "\\|")
        lines.append(
            f"| `{name}` | {format_duration(result.base_nanoseconds)} "
            f"| {format_duration(result.candidate_nanoseconds)} | {change} "
            f"| {control_change} | {result_verdict} |"
        )

    lines.extend(
        [
            "",
            "> The same-code control repeats the base measurement after the candidate. "
            "A material control change marks the result as unstable runner drift.",
            "",
            "> Informational only. Rerun small or inconclusive changes before drawing a conclusion.",
        ]
    )

    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description="Render Criterion comparisons as Markdown")
    parser.add_argument("--criterion-directory", type=Path, default=Path("target/criterion"))
    parser.add_argument("--control-directory", type=Path, required=True)
    parser.add_argument("--baseline", default="base")
    parser.add_argument("--base", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--mode", required=True)
    parser.add_argument("--noise-threshold", type=float, default=0.01)
    arguments = parser.parse_args()

    results = load_results(arguments.criterion_directory, arguments.baseline)
    controls = load_results(arguments.control_directory, arguments.baseline)
    print(
        render_markdown(
            results,
            controls,
            arguments.base,
            arguments.candidate,
            arguments.mode,
            arguments.noise_threshold,
        )
    )


if __name__ == "__main__":
    main()
