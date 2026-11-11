#!/usr/bin/env python3
"""Build all-target scalar callees and verify native leaf-frame ABI behavior."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def call(command, work, status=0, timeout=30):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=timeout)
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
    with tempfile.TemporaryDirectory(prefix='dunder-frame-free-') as temporary:
        work = Path(temporary)
        harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/frame-free-emitter.ddr', '-o', harness], work, timeout=60)
        for selected in range(11):
            status = {5: 101, 6: 102, 7: 101, 8: 101, 9: 101}.get(selected, 0)
            for target in range(4):
                results = {}
                for mode in ('off', 'on'):
                    image = work / f'{selected}-{target}-{mode}'
                    generated = call([harness, image, target, mode, selected], work)
                    words, frameless = map(int, generated.stdout.split())
                    eligible = target < 2 and selected != 10 and mode == 'on'
                    if frameless != int(eligible):
                        raise AssertionError(f'{selected}/{target}/{mode}: wrong frame policy')
                    results[mode] = (words, image.read_bytes())
                    if target == host:
                        ran = call([image], work, status)
                        if ran.stdout or ran.stderr:
                            raise AssertionError(f'unexpected target output: {ran}')
                    elif target == 3:
                        call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work,
                             125 if status else 0)
                if target < 2 and selected != 10:
                    if results['on'][0] >= results['off'][0]:
                        raise AssertionError(f'{selected}/{target}: frame omission did not shrink code')
                elif results['on'] != results['off']:
                    raise AssertionError(f'{selected}/{target}: fallback code changed')
        print('frame-free: all targets built; host/Wasm ABI, cycles, normalization, loops, spills and traps passed')


if __name__ == '__main__':
    main()
