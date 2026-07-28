import json
import tempfile
import unittest
from pathlib import Path
from typing import Optional

from scripts.benchmark_summary import BenchmarkResult, load_results, render_markdown


class BenchmarkSummaryTest(unittest.TestCase):
    def test_renders_a_significant_improvement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            criterion_directory = Path(temporary_directory)
            benchmark_directory = criterion_directory / "message_lifecycle_1_kib"

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
                {"full_id": "message_lifecycle/1_kib"},
            )

            results = load_results(criterion_directory, "base")
            controls = [
                self.result(
                    name="message_lifecycle/1_kib",
                    change=0.0,
                    lower_bound=-0.005,
                    upper_bound=0.005,
                )
            ]
            summary = render_markdown(
                results, controls, "abc123", "def456", "standard", 0.01
            )

            self.assertIn("| `message_lifecycle/1_kib` | 2.50 ms | 2.00 ms", summary)
            self.assertIn("−20.00% (−25.00% to −15.00%)", summary)
            self.assertIn("🟢 Improved", summary)

    def test_treats_a_small_change_as_inconclusive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            criterion_directory = Path(temporary_directory)
            benchmark_directory = criterion_directory / "message_lifecycle_1_kib"

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
                {"full_id": "message_lifecycle/1_kib"},
            )

            results = load_results(criterion_directory, "base")
            controls = [
                self.result(
                    name="message_lifecycle/1_kib",
                    change=0.0,
                    lower_bound=-0.005,
                    upper_bound=0.005,
                )
            ]
            summary = render_markdown(
                results, controls, "abc123", "def456", "standard", 0.01
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
            [candidate], [control], "abc123", "def456", "standard", 0.01
        )

        self.assertIn("🟠 Unstable", summary)

    def result(
        self,
        *,
        name: str,
        change: float,
        lower_bound: float,
        upper_bound: float,
    ) -> BenchmarkResult:
        return BenchmarkResult(
            name=name,
            base_nanoseconds=2_500_000,
            candidate_nanoseconds=2_000_000,
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
