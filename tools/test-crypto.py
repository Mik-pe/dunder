#!/usr/bin/env python3
"""Check native crypto ABI against AES-GCM, HMAC and HKDF published vectors."""
import argparse
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
        return TARGETS[0]
    if system == "Linux" and machine in ("arm64", "aarch64"):
        return TARGETS[1]
    if system == "Linux" and machine == "x86_64":
        return TARGETS[2]
    raise RuntimeError(f"unsupported execution host {system}/{machine}")


def run(command):
    result = subprocess.run(list(map(str, command)), cwd=ROOT,
                            capture_output=True, text=True, timeout=90)
    assert result.returncode == 0, (command, result.returncode,
                                    result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--runner", action="append", default=[], metavar="TARGET=COMMAND")
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
    with tempfile.TemporaryDirectory(prefix="dunder-crypto-") as directory:
        work = Path(directory)
        for target in TARGETS:
            for mode, flags in enumerate(MODES):
                images = []
                workers = (1, 4) if target == host == "arm64-macos" else (1,)
                for jobs in workers:
                    image = work / f"crypto-{target}-{mode}-{jobs}"
                    run([compiler, "build", ROOT / "tests/crypto.ddr", "--target", target,
                         "--jobs", jobs, *flags, "-o", image])
                    builds += 1
                    images.append(image.read_bytes())
                    if target in runners:
                        result = run([*runners[target], image])
                        lines = result.stdout.splitlines()
                        assert result.stderr == "" and len(lines) == 2, result
                        assert lines[0].startswith("$argon2id$v=19$m=19456,t=2,p=1$")
                        assert lines[1] == "crypto ok", result.stdout
                        executions += 1
                assert all(data == images[0] for data in images), (target, mode)
    print(f"crypto: {builds} native images, {executions} target executions; "
          "AES-GCM/HMAC/HKDF vectors, Argon2id, random, tampering and bounds passed")


if __name__ == "__main__":
    main()
