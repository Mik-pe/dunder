#!/usr/bin/env python3
"""Untimed three-target origin roundtrip and native host execution."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=True)
    assert not any(work.iterdir()), 'preserve earlier outcomes; choose an empty directory'
    receipt = {'protocol': 'mir-stack-origin-v1', 'timings': 'none',
               'compiler': str(compiler), 'compiler_sha256': digest(compiler),
               'commands': [], 'images': [], 'complete': False}

    def save():
        (work / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')

    def call(command):
        result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                                text=True, timeout=30)
        receipt['commands'].append({'argv': list(map(str, command)), 'exit': result.returncode,
                                    'stdout': result.stdout, 'stderr': result.stderr})
        save()
        assert result.returncode == 0, receipt['commands'][-1]
        return result

    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    emitter = work / 'emitter'
    source = ROOT / 'tests/fixtures/mir-stack-origin-emitter.ddr'
    receipt['emitter_source_sha256'] = digest(source)
    call([compiler, 'build', source, '-o', emitter])
    for target in range(3):
        images = []
        for origin in ('raw', 'temp'):
            image = work / f'valid-{target}-{origin}'
            call([emitter, image, target, origin, 'valid'])
            images.append(image)
            row = {'target': target, 'origin': origin, 'path': str(image), 'sha256': digest(image)}
            if target == host:
                result = call([image]); assert result.stdout == '42\n' and result.stderr == ''
                row['execution'] = 'host passed:42'
            else:
                row['execution'] = 'cross-built only; no execution claim'
            receipt['images'].append(row); save()
        assert images[0].read_bytes() == images[1].read_bytes(), target
    assert digest(compiler) == receipt['compiler_sha256']
    receipt['complete'] = True; save()
    print('PASS:6 exact raw/temp target images; native host success; no timings')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == '__main__':
    main()
