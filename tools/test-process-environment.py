#!/usr/bin/env python3
"""Verify native FFI environment changes across reads, spawn, PATH and exec."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import struct
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


def no_interpreter(image):
    data = image.read_bytes()
    assert data[:6] == b'\x7fELF\x02\x01'
    phoff = struct.unpack_from('<Q', data, 32)[0]
    size, count = struct.unpack_from('<HH', data, 54)
    kinds = [struct.unpack_from('<I', data, phoff + i * size)[0] for i in range(count)]
    assert 3 not in kinds and 2 not in kinds, 'pure process ELF acquired dynamic dependencies'


def check(image, pure, work):
    initial = work / 'initial path'
    changed = work / 'changed path'
    initial.mkdir(); changed.mkdir()
    # Resolving the startup PATH would select false instead of the real probe.
    (initial / 'dunder-live-environment').symlink_to('/usr/bin/false')
    (changed / 'dunder-live-environment').symlink_to(image)
    env = {**os.environ, 'PATH': f'{initial}:/usr/bin:/bin',
           'DUNDER_ENV_CHANGED': 'original', 'DUNDER_ENV_DELETED': 'delete-me'}
    env.pop('DUNDER_ENV_ADDED', None)
    env.pop('DUNDER_ENV_EMPTY', None)
    for mode in ('spawn', 'override', 'replace', 'replace-as'):
        child = subprocess.Popen([image, mode, changed], env=env,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        stdout, stderr = child.communicate(timeout=10)
        assert child.returncode == 0 and not stderr, (mode, child.returncode, stdout, stderr)
        result = json.loads(stdout)
        assert result['DUNDER_ENV_ADDED'] == ('child override' if mode == 'override' else 'added å 🌍'), result
        assert result['DUNDER_ENV_ADDED_found'] is True, result
        assert result['DUNDER_ENV_CHANGED'] == ('' if mode == 'override' else 'changed'), result
        assert result['DUNDER_ENV_CHANGED_found'] is (mode != 'override'), result
        assert result['DUNDER_ENV_DELETED'] == '' and result['DUNDER_ENV_DELETED_found'] is False, result
        assert result['DUNDER_ENV_EMPTY'] == '' and result['DUNDER_ENV_EMPTY_found'] is True, result
        assert result['argv0'] == ('live-environment-alias' if mode == 'replace-as' else 'dunder-live-environment'), result
        if mode.startswith('replace'):
            assert result['pid'] == child.pid, result
        else:
            assert result['pid'] != child.pid, result
    result = subprocess.run([image, 'unset-path', changed], env=env,
                            capture_output=True, timeout=10)
    assert result.returncode == 0 and result.stdout == b'default-path' and not result.stderr, result
    if platform.system() == 'Linux':
        child = subprocess.Popen([image, 'clear'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        stdout, stderr = child.communicate(timeout=10)
        assert child.returncode == 0 and not stderr, (child.returncode, stdout, stderr)
        spawned, replaced = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        for result in (spawned, replaced):
            for key in ('ADDED', 'CHANGED', 'DELETED', 'EMPTY'):
                assert result['DUNDER_ENV_' + key] == '' and result['DUNDER_ENV_' + key + '_found'] is False, result
        assert spawned['pid'] != child.pid and replaced['pid'] == child.pid
        assert replaced['argv0'] == 'cleared-environment'
        no_interpreter(pure)
    result = subprocess.run([pure], env=env, capture_output=True, timeout=10)
    assert result.returncode == 0 and result.stdout == b'pure inherited environment\n' and not result.stderr, result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, nargs='?')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--pure-binary', type=Path)
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    if (args.compiler is None) == (args.binary is None) or bool(args.binary) != bool(args.pure_binary):
        parser.error('provide a compiler, or both --binary and --pure-binary')
    compiler = args.compiler.resolve(strict=True) if args.compiler else None
    host = host_target(); started = time.monotonic(); builds = executions = 0
    binaries = []
    with tempfile.TemporaryDirectory(prefix='dunder-process-environment-') as temporary:
        work = Path(temporary); runnable = []
        if args.binary:
            runnable.append((args.binary.resolve(strict=True), args.pure_binary.resolve(strict=True)))
        else:
            for target in TARGETS:
                for mode, flags in enumerate(((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))):
                    images = []
                    for source in ('process_environment', 'process_environment_pure'):
                        image = work / f'{source}-{target}-{mode}'
                        built = subprocess.run([compiler, 'build', ROOT / f'tests/{source}.ddr',
                                                '--target', target, '--jobs', '1', *flags, '-o', image],
                                               cwd=ROOT, capture_output=True, text=True, timeout=90)
                        assert built.returncode == 0, (source, target, built.stdout, built.stderr)
                        images.append(image); builds += 1
                        if target.endswith('-linux') and source.endswith('_pure'):
                            no_interpreter(image)
                    if target == host:
                        runnable.append(tuple(images))
        for index, (image, pure) in enumerate(runnable):
            directory = work / f'run-{index}'; directory.mkdir()
            check(image, pure, directory); executions += 1
            binaries.append({'ffi': hashlib.sha256(image.read_bytes()).hexdigest(),
                             'pure': hashlib.sha256(pure.read_bytes()).hexdigest()})
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps({'schema': 'dunder-process-environment-verification/v1',
            'host_target': host, 'builds': builds, 'executions': executions, 'binaries': binaries,
            'seconds': round(time.monotonic() - started, 3), 'passed': True}, indent=2) + '\n')
    print(f'Process environment: {builds} builds, {executions} live executions passed')


if __name__ == '__main__':
    main()
