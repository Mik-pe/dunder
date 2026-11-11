#!/usr/bin/env python3
"""Exercise standalone inline-catalog placement without source activation."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def call(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=60)
    if result.returncode != status:
        raise AssertionError(f'{command}: {result.returncode}, expected {status}\n'
                             f'{result.stdout}{result.stderr}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-inline-catalog-') as temporary:
        work = Path(temporary); harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/inline-catalog-emitter.ddr', '-o', harness], work)
        for selected in [*range(9), 11, 12]:
            status = {3: 101, 4: 102, 5: 101, 8: 101}.get(selected, 0)
            for target in range(4):
                for mode in ('off', 'on'):
                    image = work / f'{selected}-{target}-{mode}'
                    generated = call([harness, image, target, mode, selected], work)
                    actual, estimate = map(int, generated.stdout.split())
                    if not 0 < actual <= estimate:
                        raise AssertionError('word estimate did not bound emitted staging/expression')
                    if target == host:
                        call([image], work, status)
                    elif target == 3:
                        call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work,
                             125 if status else 0)
        for refused in (9, 10):
            image = work / f'refused-{refused}'
            call([harness, image, host, 'on', refused], work)
            if image.exists():
                raise AssertionError('refused placement changed/published Code')
    print('inline catalog: all targets, rebased entries, word bounds, canonical homes, caller slots/ABI, traps and untouched fallback passed')


if __name__ == '__main__':
    main()
