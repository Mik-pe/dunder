"""Check compile-fail fixtures through the selfhost CLI without running their code."""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def check_fixture(compiler, source, work):
    expectations = [line.removeprefix("// error: ") for line in source.read_text().splitlines()
                    if line.startswith("// error: ")]
    if len(expectations) != 1 or not expectations[0]:
        raise ValueError(f"{source} needs one // error: expectation")
    previous = b"previous successful output"
    output = work / "image"
    suite = work / "suite"
    suite.mkdir(exist_ok=True)
    receipt = suite / "keep"
    receipt.write_bytes(previous)
    for target in ([], ["--target", "wasm32"]):
        modes = ["check", "build", "build-tests"] if target else ["check", "build", "build-tests", "test"]
        for command in modes:
            output.write_bytes(previous)
            options = ["--agent"] if command == "check" else ["-o", str(suite if command == "test" else output)]
            result = subprocess.run([compiler, command, source, *target, *options], cwd=work,
                                    capture_output=True, text=True, timeout=30)
            message = result.stderr
            if command == "check" and result.returncode == 1:
                error = json.loads(result.stdout)
                if (error.get("kind") != "error" or Path(error["path"]).resolve() != source or
                        not 0 <= error["start"] <= error["end"] <= source.stat().st_size):
                    raise AssertionError(f"invalid diagnostic {error}")
                message = error["message"]
            if result.returncode != 1 or expectations[0] not in message:
                raise AssertionError(f"{command} {' '.join(target)} exited {result.returncode}: {message}\n{result.stdout}")
            if output.read_bytes() != previous or receipt.read_bytes() != previous or list(suite.iterdir()) != [receipt]:
                raise AssertionError(f"{command} modified existing output")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("sources", nargs="*", type=Path, help="optional individual .ddr fixtures")
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    sources = sorted(src.resolve() for src in (args.sources or (ROOT / "tests/fixtures").glob("*_errors/*.ddr")))
    if not sources:
        parser.error("no compile-fail fixtures found")
    failed = 0
    with tempfile.TemporaryDirectory(prefix="dunder-diagnostics-") as tmp:
        for source in sources:
            try:
                check_fixture(compiler, source, Path(tmp))
                print("PASS", source.name)
            except (AssertionError, KeyError, ValueError, subprocess.TimeoutExpired) as error:
                print("FAIL", source.name, error)
                failed += 1
    print(f"{len(sources) - failed} passed, {failed} failed")
    return int(failed != 0)

if __name__ == "__main__":
    raise SystemExit(main())
