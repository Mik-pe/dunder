#!/usr/bin/env python3
"""Probe shared runtime growth emission and ordinary array values on all targets."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
INVALID_COUNTS = (-1, -2147483648, -9223372036854775808,
                  2147483648, 4294967295, 9223372036854775807)


def call(command, *, status=0, output=None, cwd=None):
    result = subprocess.run([str(x) for x in command], capture_output=True,
                            text=True, timeout=60, cwd=cwd)
    if result.returncode != status or (output is not None and result.stdout != output):
        raise AssertionError(f'{command}: status={result.returncode}, expected={status}; '
                             f'stdout={result.stdout!r}; stderr={result.stderr!r}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    executed = (host, 3)
    with tempfile.TemporaryDirectory(prefix='dunder-array-capacity-') as directory:
        work = Path(directory)
        emitter = work / 'emitter'
        # The harness imports this checkout's runtime and compiler modules, so
        # the supplied compiler need not already contain the runtime change.
        call([compiler, 'build', ROOT / 'tests/fixtures/array-capacity-emitter.ddr', '-o', emitter], cwd=work)

        def probe(name, mode, argument='', status=0, output='', repeat=False):
            for target in range(4):
                image = work / f'{name}-{target}'
                call([emitter, image, target, mode, argument], cwd=work)
                if target not in executed:
                    continue
                command = [image]
                expected = status
                if target == 3:
                    command = ['node', ROOT / 'tools/runtime-contract-wasm.cjs', image]
                    if repeat:
                        command.append('--repeat')
                    if status:
                        expected = 125
                call(command, status=expected, output=output, cwd=work)
            print(f'PASS {name}: all target images built; host/Wasm executed', flush=True)

        probe('growth-boundaries', 'math')
        for count in INVALID_COUNTS:
            for mode in ('growth-invalid', 'reserve-invalid'):
                probe(f'{mode}-{count}', mode, count, status=109)
        probe('push-at-i32-max', 'push-limit', status=109)
        probe('packed-clones-conversions', 'program', ROOT / 'tests/fixtures/array-capacity-values.ddr', output='42\n', repeat=True)
        for count in (-1, 2147483647):
            source = work / f'source-trap-{count}.ddr'
            source.write_text(f'fn main(){{var a:[]u8=[];print(41);a.reserve({count});print(42)}}\n')
            probe(f'source-capacity-trap-{count}', 'program', source, status=109, output='41\n')
    print('Managed array bounds: shared growth math, reserve/push guards and ordinary values passed; '
          'cross-native images built but not executed')


if __name__ == '__main__':
    main()
