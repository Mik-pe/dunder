#!/usr/bin/env python3
"""Verify in-place exec PID, signals, argv0, exact arguments and ambient state."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return TARGETS[0]
    if host[0] == 'Linux' and host[1] in ('arm64', 'aarch64'):
        return TARGETS[1]
    if host == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported native host {host}')


def check(image, work):
    cwd = work / 'working directory'
    cwd.mkdir(exist_ok=True)
    bin_dir = work / 'executables'
    bin_dir.mkdir(exist_ok=True)
    executable = bin_dir / 'replacement'
    if not executable.exists():
        executable.symlink_to(image)
    env = {**os.environ, 'DUNDER_REPLACE_SENTINEL': 'kept å 🌍', 'PATH': str(bin_dir)}
    payload = b'\x00\x01\x7f\xff'
    arguments = ['probe', '', 'two words', 'å 🌍', '$(touch should-not-exist)', 'x; exit 99']
    for mode, program, argv0 in (
        ('replace', str(image), str(image)),
        ('replace-as', 'replacement', 'pre-receive'),
        ('replace-as', str(image), 'snus-runner'),
    ):
        command = [str(image), mode, program]
        if mode == 'replace-as':
            command.append(argv0)
        process = subprocess.Popen([*command, *arguments], cwd=cwd, env=env,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        stdout, stderr = process.communicate(payload, timeout=5)
        assert process.returncode == 37, (command, process.returncode, stdout, stderr)
        result = json.loads(stdout)
        assert result == {'pid': process.pid, 'argv0': argv0, 'cwd': str(cwd.resolve()),
                          'environment': env['DUNDER_REPLACE_SENTINEL'],
                          'args': arguments, 'stdin_hex': payload.hex()}, result
        assert stderr == b'probe stderr\n', stderr
        assert not (cwd / 'should-not-exist').exists()
    completed = subprocess.run([image, 'replace', '/usr/bin/true'], cwd=cwd, env=env,
                               capture_output=True, timeout=5)
    assert completed.returncode == 0 and not completed.stdout and not completed.stderr, completed
    # Empty PATH and empty components mean the current directory, while an
    # absent PATH uses the standard system search path.
    (cwd / 'replacement').symlink_to(image)
    for path in ('', ':', '/definitely/missing:'):
        process = subprocess.Popen([image, 'replace', 'replacement', 'probe'], cwd=cwd,
                                   env={**env, 'PATH': path}, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        stdout, stderr = process.communicate(payload, timeout=5)
        assert process.returncode == 37, (path, process.returncode, stdout, stderr)
        assert json.loads(stdout)['pid'] == process.pid
    missing_path = {key: value for key, value in env.items() if key != 'PATH'}
    completed = subprocess.run([image, 'replace-as', 'sh', 'replacement-shell', '-c',
                                'test "$0" = replacement-shell && exit 23'],
                               env=missing_path, capture_output=True, timeout=5)
    assert completed.returncode == 23, (completed.stdout, completed.stderr)
    completed = subprocess.run([image, 'failures'], cwd=cwd, env=env, input=payload,
                               capture_output=True, timeout=5)
    assert completed.returncode == 0, (completed.returncode, completed.stdout, completed.stderr)
    assert completed.stdout == b'replace failures preserve caller\n', completed.stdout
    assert completed.stderr == b'failure stderr\n', completed.stderr
    # ENOEXEC must be returned without falling back to a shell interpreter.
    no_image = bin_dir / 'no-image'
    no_image.write_text('touch should-not-exist\n')
    no_image.chmod(0o755)
    completed = subprocess.run([image, 'replace', 'no-image', 'unused'], cwd=cwd, env=env,
                               capture_output=True, timeout=5)
    assert completed.returncode == 99 and completed.stdout == b'exec failed 8\n', completed
    assert not (cwd / 'should-not-exist').exists()
    denied = bin_dir / 'denied'
    denied.write_text('not executable\n')
    denied.chmod(0o644)
    completed = subprocess.run([image, 'replace', 'denied', 'unused'], cwd=cwd,
                               env={**env, 'PATH': f'{bin_dir}:/definitely/missing'},
                               capture_output=True, timeout=5)
    assert completed.returncode == 99 and completed.stdout == b'exec failed 13\n', completed
    # Sending SIGTERM to the original PID must terminate the replacement.
    process = subprocess.Popen([image, 'replace-as', 'replacement', 'snus-agent', 'wait'],
                               cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        ready, _, _ = select.select([process.stdout], [], [], 5)
        assert ready, 'replacement did not announce its PID'
        result = json.loads(process.stdout.readline())
        assert result['pid'] == process.pid and result['argv0'] == 'snus-agent', result
        process.send_signal(signal.SIGTERM)
        stdout, stderr = process.communicate(timeout=5)
        assert process.returncode == -signal.SIGTERM, (process.returncode, stdout, stderr)
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, nargs='?')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    if (args.compiler is None) == (args.binary is None):
        parser.error('provide either a compiler or --binary')
    compiler = args.compiler.resolve(strict=True) if args.compiler else None
    host = host_target()
    started = time.monotonic()
    builds = executions = 0
    binaries = []
    with tempfile.TemporaryDirectory(prefix='dunder-process-replace-') as temporary:
        work = Path(temporary)
        runnable = []
        if args.binary:
            runnable.append(args.binary.resolve(strict=True))
        else:
            for target in TARGETS:
                for mode, flags in enumerate(((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))):
                    image = work / f'replace-{target}-{mode}'
                    built = subprocess.run([compiler, 'build', ROOT / 'tests/process_replace.ddr',
                                            '--target', target, '--jobs', '1', *flags, '-o', image],
                                           cwd=ROOT, capture_output=True, text=True, timeout=90)
                    assert built.returncode == 0, (target, built.stdout, built.stderr)
                    builds += 1
                    if target == host:
                        runnable.append(image)
        for index, image in enumerate(runnable):
            execution_dir = work / f'run-{index}'
            execution_dir.mkdir()
            check(image, execution_dir)
            binaries.append(hashlib.sha256(image.read_bytes()).hexdigest())
            executions += 1
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps({
            'schema': 'dunder-process-replace-verification/v1', 'host_target': host,
            'builds': builds, 'executions': executions, 'binaries': binaries,
            'seconds': round(time.monotonic() - started, 3), 'passed': True,
        }, indent=2) + '\n')
    print(f'Process replace: {builds} native builds, {executions} live executions passed')


if __name__ == '__main__':
    main()
