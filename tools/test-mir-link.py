#!/usr/bin/env python3
"""Ordered FunctionObject linking: serial equality and actual target execution."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')


def call(command, root, log):
    command = list(map(str, command))
    result = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=120)
    log.append({'command': command, 'exit': result.returncode,
                'stdout': result.stdout, 'stderr': result.stderr})
    assert result.returncode == 0, log[-1]
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, default=ROOT / 'build/mir-link-qa')
    args = parser.parse_args()
    root = args.source_root.resolve(); compiler = args.compiler.resolve(strict=True)
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=True)
    host = {'Darwin': {'arm64': 0}, 'Linux': {'aarch64': 1, 'arm64': 1, 'x86_64': 2}}[platform.system()][platform.machine()]
    log, images = [], []; executions = 0
    try:
        call([compiler, 'test', 'tests/mir_link.ddr', '-o', output/'unit'], root, log)
        for target in TARGETS:
            image = output/('unit-'+target)
            call([compiler, 'build-tests', 'tests/mir_link.ddr', '--target', target, '-o', image], root, log)
            if target == 'wasm32': call(['node', 'tools/test-wasm.mjs', '--execute', image], root, log)
        emitter = output/'emitter'
        call([compiler, 'build', 'tests/fixtures/mir-link-emitter.ddr', '-o', emitter], root, log)
        for target, name in enumerate(TARGETS):
            for optimized in ('off', 'on'):
                for schedule in ('grow', 'sealed'):
                    pair = []
                    for mode in ('serial', 'link'):
                        image = output/f'{name}-{optimized}-{schedule}-{mode}'
                        assert call([emitter, image, target, mode, optimized, schedule], root, log).strip() == 'objects=3'
                        data = image.read_bytes(); pair.append(data)
                        images.append({'target': name, 'optimization': optimized, 'schedule': schedule,
                                       'mode': mode, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
                        if target == host:
                            call([image], root, log); executions += 1
                        elif target == 3:
                            call(['node', 'tools/runtime-contract-wasm.cjs', image], root, log); executions += 1
                    assert pair[0] == pair[1], (name, optimized, schedule, 'serial/link mismatch')
        report = {'result': 'passed', 'images': images, 'executions': executions,
                  'scope': 'inactive ordered relocatable linker; no threads/frontends activated',
                  'imports': 'all-target Code/site contracts; symbol slots remain caller-owned',
                  'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(),
                  'source_sha256': {p: hashlib.sha256((root/p).read_bytes()).hexdigest() for p in (
                      'selfhost/mir_link.ddr', 'tests/mir_link.ddr', 'tests/fixtures/mir-link-emitter.ddr')}}
        (output/'summary.json').write_text(json.dumps(report, indent=2)+'\n')
        print(f'PASS ordered linker: {len(images)} images, {executions} native/Wasm executions; serial equality and atomic rejection contracts')
    finally:
        (output/'commands.json').write_text(json.dumps(log, indent=2)+'\n')


if __name__ == '__main__':
    main()
