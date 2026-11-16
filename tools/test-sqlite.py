#!/usr/bin/env python3
"""Compile SQLite contracts on native targets and run available target runtimes."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ("arm64-macos", "arm64-linux", "x86_64-linux")
MODES = ((), ("--scalar-opt=off", "--ownership-opt=off", "--mir=off"))


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == "Darwin" and machine == "arm64":
        return "arm64-macos"
    if system == "Linux" and machine in ("arm64", "aarch64"):
        return "arm64-linux"
    if system == "Linux" and machine == "x86_64":
        return "x86_64-linux"
    raise RuntimeError(f"unsupported execution host {system}/{machine}")


def run(command):
    result = subprocess.run(list(map(str, command)), cwd=ROOT,
                            capture_output=True, text=True, timeout=90)
    assert result.returncode == 0, (command, result.returncode,
                                    result.stdout, result.stderr)
    return result


def verify_output(result):
    assert result.stderr == "", result.stderr
    lines = result.stdout.splitlines()
    assert len(lines) == 4 and lines[-1] == "sqlite ok", result.stdout
    rows = [json.loads(line) for line in lines[:-1]]
    assert rows == [
        {"id": 1, "value": "åäö 雪\0after", "number": 9223372036854775807,
         "real": 1.2345678901234567, "absent": None},
        {"id": 2, "value": "", "number": None, "real": None, "absent": None},
        {"id": 3, "value": "x'); DROP TABLE items; --", "number": None,
         "real": None, "absent": None},
    ], rows
    assert type(rows[0]["number"]) is int and type(rows[0]["real"]) is float


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--runner", action="append", default=[],
                        metavar="TARGET=COMMAND",
                        help="execute another target through a wrapper or emulator")
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition("=")
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error("--runner requires native TARGET=COMMAND")
        runners[target] = shlex.split(command)
    builds = executions = 0
    with tempfile.TemporaryDirectory(prefix="dunder-sqlite-") as temporary:
        work = Path(temporary)
        for target in TARGETS:
            for mode, flags in enumerate(MODES):
                workers = (1, 4) if target == host == "arm64-macos" else (1,)
                images = []
                for jobs in workers:
                    image = work / f"sqlite-{target}-{mode}-{jobs}"
                    run([compiler, "build", ROOT / "tests/sqlite.ddr",
                         "--target", target, "--jobs", jobs, *flags, "-o", image])
                    images.append(image.read_bytes())
                    builds += 1
                    if target in runners:
                        database = work / (image.name + ".db")
                        result = run([*runners[target], image, database])
                        verify_output(result)
                        executions += 1
                assert all(data == images[0] for data in images), (target, mode)
    print(f"sqlite: {builds} native images, {executions} target executions; "
          "parameter binding, exact types/UTF-8, statement boundaries, triggers, "
          "read-only handles, limits and cleanup passed")


if __name__ == "__main__":
    main()
