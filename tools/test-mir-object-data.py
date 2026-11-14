#!/usr/bin/env python3
"""Numeric object transport: malformed inputs, single walker and task lifetime.

Unit contracts execute on the native host; all three native targets are built.
ARM64/macOS additionally runs the actual isolated-task generator, compares
serial/task-produced images at capacities 1/2/4, and executes native
kernels after the child pools and scopes have been reclaimed. Other hosts
explicitly omit this unsupported source-thread path. No timing measurements.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, default=ROOT / 'build/mir-object-data-qa')
    args = parser.parse_args()
    root = args.source_root.resolve(strict=True)
    compiler = args.compiler.resolve(strict=True)
    host = {'Darwin': {'arm64': 0}, 'Linux': {'arm64': 1, 'aarch64': 1, 'x86_64': 2}}[platform.system()][platform.machine()]
    args.output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='run-', dir=args.output.resolve()))
    commands, images, units = [], [], []
    groups = {}
    executed = {TARGETS[host]: 0}
    passed = False

    def call(command, expected=None):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=root, text=True, capture_output=True, timeout=180)
        commands.append({'command': command, 'exit': result.returncode,
                         'stdout': result.stdout, 'stderr': result.stderr})
        if result.returncode or (expected is not None and result.stdout != expected):
            raise AssertionError(commands[-1])
        return result.stdout

    try:
        unit = root / 'tests/mir_object_data.ddr'
        output = call([compiler, 'test', unit, '-o', work / 'unit-native'])
        match = re.search(r'(\d+) passed, 0 failed', output)
        assert match, output
        groups[TARGETS[host]] = int(match[1])
        listing = call([compiler, 'build-tests', unit, '--list']).splitlines()
        assert len(listing) == groups[TARGETS[host]], listing
        for target in TARGETS:
            image = work / f'unit-{target}-0'
            call([compiler, 'build-tests', unit, '--target', target, '--group', 0, '-o', image])
            units.append({'target': target, 'group': 0, 'sha256': digest(image)})
        if host == 0:
            emitter = work / 'task-emitter'
            call([compiler, 'build', root / 'tests/fixtures/mir-object-data-task.ddr', '-o', emitter])
            for target, name in enumerate(TARGETS):
                reference = None
                for capacity in (1, 2, 4):
                    for mode in ('serial', 'task'):
                        image = work / f'{name}-{capacity}-{mode}'
                        call([emitter, image, target, mode, capacity],
                             'objects=1;lowering=worker-or-serial-once;iterations=3\n')
                        sha = digest(image)
                        if reference is None:
                            reference = sha
                        assert reference == sha, (name, capacity, mode, 'serial/task image mismatch')
                        images.append({'target': name, 'capacity': capacity, 'mode': mode,
                                       'sha256': sha, 'bytes': image.stat().st_size})
                        if target == host:
                            call([image], ''); executed[name] += 1
        passed = True
    finally:
        (work / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
        (work / 'summary.json').write_text(json.dumps({
            'result': 'passed' if passed else 'failed', 'compiler_sha256': digest(compiler),
            'source_sha256': {name: digest(root / name) for name in (
                'selfhost/mir_lower.ddr', 'selfhost/mir_task.ddr', 'selfhost/compile_parallel.ddr',
                'tests/mir_object_data.ddr', 'tests/fixtures/mir-object-data-task.ddr',
                'tools/test-mir-object-data.py')},
            'unit_groups': groups, 'unit_images': units, 'kernel_images': images,
            'kernel_executions': executed, 'host_target': TARGETS[host],
            'task_scope_probe': 'actual native source tasks' if host == 0 else 'not supported on this host; omitted',
            'provenance': 'exact task scope, ordinal and declaration remain trusted; structural import is not a semantic byte certificate',
            'lifetime': ('repeated await mutation/cache isolation, discard, refused job, scope reentry, accepted object survives child-pool and scope cleanup'
                         if host == 0 else 'not run; source threads unsupported on this host'),
            'linux_execution': 'cross-built only' if host == 0 else 'native unit driver only; no source threads',
        }, indent=2) + '\n')
        print(f'{"PASS" if passed else "FAIL"} numeric object transport: {work}')


if __name__ == '__main__':
    main()
