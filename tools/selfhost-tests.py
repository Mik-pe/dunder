#!/usr/bin/env python3
"""Run each group from a Dunder build-tests executable in a fresh process."""

import argparse
import os
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("suite", help="executable produced by dunder build-tests")
    parser.add_argument("--timeout", type=float, default=5.0, help="seconds per group")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    suite = os.path.abspath(args.suite)
    listing = subprocess.run([suite, "--list"], check=True, capture_output=True,
                             timeout=args.timeout, text=True)
    passed = failed = 0
    for index, line in enumerate(listing.stdout.splitlines()):
        group, separator, name = line.partition(" ")
        if group != str(index) or not separator or not name:
            raise ValueError(f"invalid group listing: {line!r}")
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            try:
                result = subprocess.run([suite, group], stdout=stdout, stderr=stderr,
                                        stdin=subprocess.DEVNULL, timeout=args.timeout)
            except subprocess.TimeoutExpired:
                print(f"FAIL {name} (timeout)")
                failed += 1
                continue
            if result.returncode == 0:
                print(f"PASS {name}")
                passed += 1
            else:
                print(f"FAIL {name} (exit {result.returncode})")
                for stream in (stdout, stderr):
                    stream.seek(0)
                    output = stream.read(4096)
                    if output:
                        print(output.decode(errors="replace").rstrip())
                failed += 1
    print(f"{passed} passed, {failed} failed")
    return int(failed > 0)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f"error running test suite: {error}")
