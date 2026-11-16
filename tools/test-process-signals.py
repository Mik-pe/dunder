#!/usr/bin/env python3
"""Verify spawned signal delivery without unblocking the coordinating process."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


def host_target():
    system, machine = platform.system(), platform.machine()
    if (system, machine) == ('Darwin', 'arm64'):
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if (system, machine) == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported host {system}/{machine}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, nargs='?')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    if (args.compiler is None) == (args.binary is None):
        parser.error('provide a compiler or --binary')
    host = host_target()
    builds = 0
    executed = []
    with tempfile.TemporaryDirectory(prefix='dunder-process-signals-') as temporary:
        work = Path(temporary)
        runnable = []
        if args.binary:
            runnable.append(args.binary.resolve(strict=True))
        else:
            compiler = args.compiler.resolve(strict=True)
            for target in TARGETS:
                for mode, flags in enumerate(((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))):
                    image = work / f'signals-{target}-{mode}'
                    built = subprocess.run([compiler, 'build', ROOT / 'tests/process_signal_mask.ddr',
                                            '--target', target, '--jobs', '1', *flags, '-o', image],
                                           cwd=ROOT, capture_output=True, text=True, timeout=90)
                    assert built.returncode == 0, (built.stdout, built.stderr)
                    builds += 1
                    if target == host:
                        runnable.append(image)
        for image in runnable:
            result = subprocess.run([image], capture_output=True, text=True, timeout=10)
            assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
            assert result.stdout == 'process signal masks ok\n' and not result.stderr, result
            executed.append(hashlib.sha256(image.read_bytes()).hexdigest())
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps({'schema':'dunder-process-signal-verification/v1',
            'host_target':host, 'builds':builds, 'executions':len(executed),
            'binaries':executed, 'passed':True}, indent=2) + '\n')
    print(f'Process signals: {builds} native builds, {len(executed)} live executions passed')


if __name__ == '__main__':
    main()
