#!/usr/bin/env python3
"""Detached FunctionObject contracts; no timing benchmarks.

The object consumer links three functions in producer order after lowering in
completion order 3,1,2 and destroying all input MIR/source arenas. It compares
all Code columns with the serial adapter before finalizing actual target images.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')


def run(command, root, log):
    command = [str(x) for x in command]
    result = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=180)
    log.append({'command': command, 'exit': result.returncode,
                'stdout': result.stdout, 'stderr': result.stderr})
    if result.returncode:
        raise AssertionError(json.dumps(log[-1], indent=2))
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, default=ROOT / 'build/mir-object-qa')
    args = parser.parse_args()
    root = args.source_root.resolve()
    compiler = args.compiler.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    system = platform.system()
    machine = platform.machine().lower()
    host = {'Darwin': {'arm64': 0}, 'Linux': {'aarch64': 1, 'arm64': 1, 'x86_64': 2}}[system][machine]
    log = []
    images = []
    executions = 0
    try:
        run([compiler, 'test', 'tests/mir_object.ddr', '-o', output / 'unit'], root, log)
        for target in TARGETS:
            unit = output / ('unit-' + target)
            run([compiler, 'build-tests', 'tests/mir_object.ddr', '--target', target, '-o', unit], root, log)
            if target == 'wasm32':
                run(['node', 'tools/test-wasm.mjs', '--execute', unit], root, log)
        emitter = output / 'emitter'
        run([compiler, 'build', 'tests/fixtures/mir-object-emitter.ddr', '-o', emitter], root, log)
        for target, name in enumerate(TARGETS):
            for optimized in ('off', 'on'):
                pair = []
                for mode in ('serial', 'object'):
                    image = output / f'{name}-{optimized}-{mode}'
                    assert run([emitter, image, target, mode, optimized], root, log).strip() == 'objects=3'
                    data = image.read_bytes()
                    pair.append(data)
                    images.append({'target': name, 'optimization': optimized, 'mode': mode,
                                   'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)})
                    if target == host:
                        run([image], root, log)
                        executions += 1
                    elif target == 3:
                        run(['node', 'tools/runtime-contract-wasm.cjs', image], root, log)
                        executions += 1
                assert pair[0] == pair[1], (name, optimized, 'serial/object image mismatch')
        summary = {'result': 'passed', 'images': images, 'executions': executions,
                   'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(),
                   'source_sha256': {path: hashlib.sha256((root / path).read_bytes()).hexdigest()
                                     for path in ('selfhost/mir_lower.ddr', 'tests/mir_object.ddr',
                                                  'tests/mir_object_support.ddr', 'tests/fixtures/mir-object-emitter.ddr')}}
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(f'PASS detached objects: {len(images)} all-target images, {executions} native/Wasm executions; unit contracts passed')
    finally:
        (output / 'commands.json').write_text(json.dumps(log, indent=2) + '\n')


if __name__ == '__main__':
    main()
