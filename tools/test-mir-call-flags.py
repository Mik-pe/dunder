#!/usr/bin/env python3
"""Raw calls may consume incoming NZCV: real CSET observers on the native host.

The actual value pass is productive but currently retains Compare even with
the old unsound flags2 model. This is NOT a production value-pass miscompile.
The separately named liveness-control mode is test-only Compare deletion: it
demonstrates why falsely dead flag metadata would license an invalid future
transformation. With canonical flags3, that control must retain Compare too.
All three native targets are built; only the native host executes. No timing tests.
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
    parser.add_argument('--expected-call-flags', type=int, choices=(2, 3), default=3,
                        help='2 records the pre-fix contract; normal regression requires3')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/mir-call-flags')
    args = parser.parse_args()
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    compiler = args.compiler.resolve(strict=True)
    base = args.output.resolve(); base.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='run-', dir=base))
    commands, images = [], []
    executions = {TARGETS[host]: 0}
    passed = False

    def call(command, stdout=None):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=120)
        row = {'command': command, 'exit': result.returncode,
               'stdout': result.stdout, 'stderr': result.stderr}
        commands.append(row)
        if result.returncode != 0 or (stdout is not None and result.stdout != stdout):
            raise AssertionError(row)
        return result.stdout

    try:
        emitter = work / 'emitter'
        fixture = ROOT / 'tests/fixtures/mir-call-flags-emitter.ddr'
        call([compiler, 'build', fixture, '-o', emitter])
        observed_live = args.expected_call_flags == 3
        for target, name in enumerate(TARGETS):
            for form in ('direct', 'indirect'):
                for equal in (0, 1):
                    for mode in ('off', 'value', 'liveness-control'):
                        image = work / f'{name}-{form}-{equal}-{mode}'
                        lines = call([emitter, image, target, form, mode, equal]).splitlines()
                        if len(lines) != 12:
                            raise AssertionError(lines)
                        flags = int(lines[0]); before_live = lines[1] == 'true'
                        after_live = lines[2] == 'true'; removed = lines[3] == 'true'
                        changed = lines[4] == 'true'; deleted = int(lines[5])
                        masks = tuple(map(int, lines[6:9])); abi = int(lines[9])
                        compare_flags = int(lines[10]); stale_rejected = lines[11] == 'true'
                        drop_control = mode == 'liveness-control' and not observed_live
                        if (flags, before_live, after_live, removed) != (
                                args.expected_call_flags, observed_live, observed_live, drop_control):
                            raise AssertionError((name, form, mode, lines))
                        if masks != (-1, 0, -1) or abi != 1 or compare_flags != 2 or not stale_rejected:
                            raise AssertionError(('raw contract or exact retained verification changed', lines))
                        if mode == 'value' and (not changed or deleted < 1):
                            raise AssertionError(('real value pass must do actual work', lines))
                        expected = 1 - equal if drop_control else equal
                        images.append({'target': name, 'form': form, 'equal': equal, 'mode': mode,
                                       'call_flags': flags, 'compare_flags_live': before_live,
                                       'compare_removed': removed, 'value_deleted': deleted,
                                       'expected_observer_result': expected, 'sha256': digest(image)})
                        if target == host:
                            call([image], str(expected) + '\n'); executions[name] += 1
        passed = True
    finally:
        (work / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
        (work / 'summary.json').write_text(json.dumps({
            'result': 'passed' if passed else 'failed', 'expected_call_flags': args.expected_call_flags,
            'compiler_sha256': digest(compiler),
            'source_sha256': {p: digest(ROOT / p) for p in (
                'selfhost/mir_decode.ddr', 'selfhost/mir_retained.ddr', 'selfhost/mir_liveness.ddr',
                'selfhost/mir_value_prop.ddr', 'tests/fixtures/mir-call-flags-emitter.ddr')},
            'images': images, 'executions': executions, 'host_target': TARGETS[host],
            'cross_built_only': [name for target, name in enumerate(TARGETS) if target != host],
            'production_value_pass': 'productive; Compare retained, observer result preserved before and after fix',
            'liveness_control': 'test-only demonstration; not a production pass or claimed production miscompile',
        }, indent=2) + '\n')
        print(('PASS' if passed else 'FAIL') + ' raw call flags: ' + str(work))


if __name__ == '__main__':
    main()
