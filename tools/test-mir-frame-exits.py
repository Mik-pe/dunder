#!/usr/bin/env python3
"""Frame promotion must preserve external and unknown-call observations.

Linux images are cross-built only on this ARM64/macOS observer host.
Optional frozen before-patch emitters make the original register/frame and
call-SP counterexamples part of the receipt.
No latency measurements or production compilation flags are changed.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
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
    parser.add_argument('--before-emitter', type=Path, help='frozen unpatched external-observer emitter')
    parser.add_argument('--before-call-sp-emitter', type=Path, help='frozen unpatched call-SP emitter')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/mir-frame-exits')
    args = parser.parse_args()
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        parser.error('native observer proof requires ARM64/macOS; Linux is image-only')
    compiler = args.compiler.resolve(strict=True)
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='run-', dir=output))
    commands, images, observations = [], [], []
    passed = False

    def call(command, expected=0, stdout=None):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=120)
        row = {'command': command, 'exit': result.returncode,
               'stdout': result.stdout, 'stderr': result.stderr}
        commands.append(row)
        if result.returncode != expected or (stdout is not None and result.stdout != stdout):
            raise AssertionError(row)
        return result

    try:
        call([compiler, 'test', 'tests/mir_frame_exits.ddr', '-o', work / 'units'])
        for target in TARGETS:
            image = work / ('units-' + target)
            call([compiler, 'build-tests', 'tests/mir_frame_exits.ddr', '--target', target, '-o', image])
        for case in ('observer', 'call-sp'):
            emitter = work / (case + '-emitter')
            call([compiler, 'build', 'tests/fixtures/mir-frame-exit-' + case + '.ddr', '-o', emitter])
            for view in (('',) if case == 'observer' else ('sp', 'gp')):
                expected = '333\n13\n' if case == 'observer' else ('16\n' if view == 'sp' else '333\n')
                snapshots = []
                for mode in ('off', 'raw', 'memory'):
                    image = work / (case + '-' + view + '-' + mode)
                    call([emitter, image, mode, view])
                    actual = call([image], stdout=expected)
                    snapshots.append({'mode': mode, 'stdout': actual.stdout, 'sha256': digest(image)})
                observations.append({'case': case, 'view': view, 'patched': snapshots})
        for case, view, before, expected in (
            ('observer', '', args.before_emitter, '13\n7\n'),
            ('call-sp', 'sp', args.before_call_sp_emitter, '32\n'),
            ('call-sp', 'gp', args.before_call_sp_emitter, '13\n'),
        ):
            if before is None:
                continue
            before = before.resolve(strict=True)
            for mode in ('raw', 'memory'):
                image = work / ('before-' + case + '-' + view + '-' + mode)
                call([before, image, mode, view])
                result = call([image], stdout=expected)
                observations.append({'case': case, 'view': view, 'before_mode': mode,
                                     'emitter_sha256': digest(before), 'stdout': result.stdout})
        terminal = work / 'terminal-emitter'
        call([compiler, 'build', 'tests/fixtures/mir-frame-exit-terminal.ddr', '-o', terminal])
        for target, name in enumerate(TARGETS):
            for taken in (0, 1):
                for policy in ('raw', 'source'):
                    image = work / f'terminal-{name}-{taken}-{policy}'
                    call([terminal, image, target, taken, policy], stdout='1\n')
                    images.append({'target': name, 'taken': taken, 'memory_policy': policy,
                                   'sha256': digest(image)})
                    if target == 0:
                        call([image])
        sync = work / 'call-sync-emitter'
        call([compiler, 'build', 'tests/fixtures/mir-frame-exit-call-sync.ddr', '-o', sync])
        for target, name in enumerate(TARGETS):
            for policy in ('raw', 'source'):
                for mode in ('off', 'on'):
                    image = work / f'call-sync-{name}-{policy}-{mode}'
                    call([sync, image, target, policy, mode], stdout='1\n' if mode == 'on' else '0\n')
                    images.append({'target': name, 'call_sync': mode, 'memory_policy': policy,
                                   'sha256': digest(image)})
                    if target == 0:
                        call([image])
        passed = True
    finally:
        (work / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
        (work / 'summary.json').write_text(json.dumps({
            'result': 'passed' if passed else 'failed', 'compiler_sha256': digest(compiler),
            'source_sha256': {name: digest(ROOT / name) for name in (
                'selfhost/mir_opt.ddr', 'selfhost/mir_terminals.ddr', 'selfhost/mir_call_abi.ddr',
                'tests/mir_frame_exits.ddr', 'tests/fixtures/mir-frame-exit-support.ddr',
                'tests/fixtures/mir-frame-exit-observer.ddr', 'tests/fixtures/mir-frame-exit-call-sp.ddr',
                'tests/fixtures/mir-frame-exit-terminal.ddr', 'tests/fixtures/mir-frame-exit-call-sync.ddr')},
            'observations': observations, 'target_images': images,
            'linux_execution': 'none; cross-built images only',
            'scope': 'external-exit and incoming-call-SP correctness; no latency measurements',
        }, indent=2) + '\n')
        print(('PASS' if passed else 'FAIL') + ' MIR frame exits: ' + str(work))


if __name__ == '__main__':
    main()
