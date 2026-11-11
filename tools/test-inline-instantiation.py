#!/usr/bin/env python3
"""Verify bounded IR template clones through the ordinary scalar emitter."""
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
    traps = {3: 101, 4: 102, 5: 101, 7: 101, 13: 101, 14: 102, 17: 101, 21: 101}
    counts = {8: 29, 16: 32, 18: 1}
    with tempfile.TemporaryDirectory(prefix='dunder-inline-instantiation-') as temporary:
        work = Path(temporary); harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/inline-instantiation-emitter.ddr', '-o', harness], work)
        for selected in range(22):
            status = traps.get(selected, 0)
            for target in range(4):
                for division in ('off', 'on'):
                    for folding in ('off', 'on'):
                        image = work / f'{selected}-{target}-{division}-{folding}'
                        result = call([harness, image, target, selected, division, folding], work)
                        nodes, params, returned = map(int, result.stdout.split())
                        if not 1 <= nodes <= 32 or not 0 <= params <= 8 or not 1 <= returned <= 3:
                            raise AssertionError(f'invalid clone/signature sizes: {result.stdout}')
                        if selected in counts and nodes != counts[selected]:
                            raise AssertionError(f'case {selected}: {nodes} nodes, expected {counts[selected]}')
                        if target == host:
                            call([image], work, status)
                        if target == 3:
                            call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work,
                                 125 if status else 0)
            print(f'PASS {selected}: all four targets; native/Wasm executed; folding and constant division off/on', flush=True)
    print('IR template instantiation: 22 cases, 352 images, typed snapshots, pre/post verification, outputs and ordered traps passed')


if __name__ == '__main__':
    main()
