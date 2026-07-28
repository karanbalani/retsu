import json
import tempfile
import unittest
from pathlib import Path
from typing import Optional

from scripts.benchmark_summary import (
    BenchmarkPair,
    BenchmarkResult,
    load_results,
    render_markdown,
)


class BenchmarkSummaryTest(unittest.TestCase):
    def test_renders_a_significant_improvement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            criterion_directory = Path(temporary_directory)
            benchmark_directory = (
                criterion_directory / "queue_operations" / "lifecycle_1_kib"
            )

            self.write_estimate(benchmark_directory / "base", 2_500_000)
            self.write_estimate(benchmark_directory / "new", 2_000_000)
            self.write_estimate(
                benchmark_directory / "change",
                -0.2,
                lower_bound=-0.25,
                upper_bound=-0.15,
            )
            self.write_json(
                benchmark_directory / "new" / "benchmark.json",
                {
                    "full_id": "queue_operations/lifecycle/1_kib",
                    "throughput": None,
                },
            )

            results = load_results(criterion_directory, "base")
            controls = [
                self.result(
                    name="queue_operations/lifecycle/1_kib",
                    change=0.0,
                    lower_bound=-0.005,
                    upper_bound=0.005,
                )
            ]
            summary = render_markdown(
                [BenchmarkPair("Pair 1", results, controls)],
                "abc123",
                "def456",
                "standard",
                0.01,
            )

            self.assertIn(
                "| `queue_operations/lifecycle/1_kib` | 2.50 ms → 2.00 ms",
                summary,
            )
            self.assertIn("−20.00% (−25.00% to −15.00%)", summary)
            self.assertIn("🟢 Improved", summary)
            self.assertIn(
                "Outcomes: 1 improved · 0 regressed · 0 inconclusive · 0 unstable",
                summary,
            )

    def test_treats_a_small_change_as_inconclusive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            criterion_directory = Path(temporary_directory)
            benchmark_directory = (
                criterion_directory / "queue_operations" / "lifecycle_1_kib"
            )

            self.write_estimate(benchmark_directory / "base", 2_500_000)
            self.write_estimate(benchmark_directory / "new", 2_487_500)
            self.write_estimate(
                benchmark_directory / "change",
                -0.005,
                lower_bound=-0.02,
                upper_bound=0.01,
            )
            self.write_json(
                benchmark_directory / "new" / "benchmark.json",
                {
                    "full_id": "queue_operations/lifecycle/1_kib",
                    "throughput": None,
                },
            )

            results = load_results(criterion_directory, "base")
            controls = [
                self.result(
                    name="queue_operations/lifecycle/1_kib",
                    change=0.0,
                    lower_bound=-0.005,
                    upper_bound=0.005,
                )
            ]
            summary = render_markdown(
                [BenchmarkPair("Pair 1", results, controls)],
                "abc123",
                "def456",
                "standard",
                0.01,
            )

            self.assertIn("⚪ Inconclusive", summary)

    def test_marks_a_result_unstable_when_same_code_drifts(self) -> None:
        candidate = self.result(
            name="message_lifecycle/1_kib",
            change=-0.2,
            lower_bound=-0.25,
            upper_bound=-0.15,
        )
        control = self.result(
            name="message_lifecycle/1_kib",
            change=0.15,
            lower_bound=0.12,
            upper_bound=0.18,
        )

        summary = render_markdown(
            [BenchmarkPair("Pair 1", [candidate], [control])],
            "abc123",
            "def456",
            "standard",
            0.01,
        )

        self.assertIn("🟠 Unstable", summary)

    def test_renders_concurrent_throughput(self) -> None:
        candidate = self.result(
            name="concurrent_lifecycle/4_workers/1_kib",
            change=-0.2,
            lower_bound=-0.25,
            upper_bound=-0.15,
            throughput_elements=4,
        )
        control = self.result(
            name="concurrent_lifecycle/4_workers/1_kib",
            change=0.0,
            lower_bound=-0.005,
            upper_bound=0.005,
            throughput_elements=4,
        )

        summary = render_markdown(
            [BenchmarkPair("Pair 1", [candidate], [control])],
            "abc123",
            "def456",
            "standard",
            0.01,
        )

        self.assertIn("2.50 ms (1,600 ops/s)", summary)
        self.assertIn("2.00 ms (2,000 ops/s)", summary)

    def test_requires_stable_pairs_to_agree(self) -> None:
        improved = self.result(
            name="queue_operations/lifecycle/1_kib",
            change=-0.2,
            lower_bound=-0.25,
            upper_bound=-0.15,
        )
        inconclusive = self.result(
            name="queue_operations/lifecycle/1_kib",
            change=-0.005,
            lower_bound=-0.02,
            upper_bound=0.01,
        )
        stable_control = self.result(
            name="queue_operations/lifecycle/1_kib",
            change=0.0,
            lower_bound=-0.005,
            upper_bound=0.005,
        )

        summary = render_markdown(
            [
                BenchmarkPair("Pair 1", [improved], [stable_control]),
                BenchmarkPair("Pair 2", [inconclusive], [stable_control]),
            ],
            "abc123",
            "def456",
            "standard",
            0.01,
        )

        self.assertIn("Comparison pairs: `2`", summary)
        self.assertIn(
            "| `queue_operations/lifecycle/1_kib` "
            "| 2.50 ms → 2.00 ms<br>−20.00%",
            summary,
        )
        result_row = next(
            line
            for line in summary.splitlines()
            if line.startswith("| `queue_operations/lifecycle/1_kib`")
        )
        self.assertTrue(result_row.endswith("| ⚪ Inconclusive |"))

    def result(
        self,
        *,
        name: str,
        change: float,
        lower_bound: float,
        upper_bound: float,
        throughput_elements: Optional[int] = None,
    ) -> BenchmarkResult:
        return BenchmarkResult(
            name=name,
            base_nanoseconds=2_500_000,
            candidate_nanoseconds=2_000_000,
            throughput_elements=throughput_elements,
            change=change,
            change_lower_bound=lower_bound,
            change_upper_bound=upper_bound,
        )

    def write_estimate(
        self,
        directory: Path,
        point_estimate: float,
        *,
        lower_bound: Optional[float] = None,
        upper_bound: Optional[float] = None,
    ) -> None:
        lower_bound = point_estimate if lower_bound is None else lower_bound
        upper_bound = point_estimate if upper_bound is None else upper_bound
        self.write_json(
            directory / "estimates.json",
            {
                "mean": {
                    "point_estimate": point_estimate,
                    "confidence_interval": {
                        "lower_bound": lower_bound,
                        "upper_bound": upper_bound,
                    },
                }
            },
        )

    def write_json(self, path: Path, value: dict) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
