"""Time fresh CLI processes against a deterministic 20-group Dunder project."""

import argparse
import csv
import math
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("report", type=Path)
    parser.add_argument("--rounds", type=int, default=30)
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("rounds must be positive")
    compiler = args.compiler.resolve(strict=True)
    cases = (
        ("check-agent", ["check", "main.ddr", "--agent"]),
        ("one-affected", ["test", "main.ddr", "--affected"]),
        ("no-change", ["test", "main.ddr", "--affected"]),
    )
    samples = {name: [] for name, _ in cases}
    with tempfile.TemporaryDirectory() as project, args.report.open("w", newline="") as report:
        writer = csv.writer(report)
        writer.writerow(["round", "case", "elapsed_ms"])
        for index in range(args.rounds + 1):
            # One nonzero constant changes in the first test's transitive callee.
            functions = [f"fn value{n}()->i32{{{1 + (index if n == 0 else n)}}}" for n in range(20)]
            tests = [f"@test fn group{n}(){{assert(value{n}()>0)}}" for n in range(20)]
            Path(project, "main.ddr").write_text("\n".join(["fn main(){}", *functions, *tests]))
            for name, flags in cases:
                start = time.perf_counter()
                result = subprocess.run([compiler, *flags], cwd=project, check=True, capture_output=True, text=True)
                elapsed = (time.perf_counter() - start) * 1000
                if not index:
                    continue
                if name == "one-affected" and not {"1 passed, 0 failed", "19 unaffected"}.issubset(result.stdout.splitlines()):
                    raise RuntimeError(result.stdout)
                if name == "no-change" and "20 unaffected" not in result.stdout.splitlines():
                    raise RuntimeError(result.stdout)
                writer.writerow([index, name, f"{elapsed:.3f}"])
                samples[name].append(elapsed)
    for name, values in samples.items():
        p95 = sorted(values)[math.ceil(len(values) * .95) - 1]
        print(f"{name} median={statistics.median(values):.3f}ms p95={p95:.3f}ms")


if __name__ == "__main__":
    main()
