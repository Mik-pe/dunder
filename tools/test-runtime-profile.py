#!/usr/bin/env python3
"""Verify native allocation-event attribution and allocation-free reporting."""
import argparse
import json
import os
from pathlib import Path
import platform
import struct
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures'
TARGETS = {'arm64-macos': 0, 'arm64-linux': 1, 'x86_64-linux': 2}
COUNTERS = (
    'allocation_count', 'requested_bytes', 'size_class_payload_bytes',
    'fresh_allocations', 'reused_allocations', 'retain_calls', 'release_calls',
    'array_clone_calls', 'array_clone_bytes', 'string_slice_calls',
    'string_slice_bytes', 'record_construct_calls', 'record_construct_bytes',
    'record_clone_calls', 'record_clone_bytes', 'copy_bytes',
)


def call(args, *, cwd, status=0, **options):
    result = subprocess.run([str(x) for x in args], cwd=cwd, capture_output=True,
                            timeout=60, **options)
    assert result.returncode == status, (args, result.returncode, result.stdout, result.stderr)
    return result


def counters(stderr):
    lines = stderr.splitlines()
    assert len(lines) == 1, stderr
    record = json.loads(lines[0])
    assert record['kind'] == 'runtime_profile' and record['version'] == 1, record
    assert record['units'] == {'counts': 'events', 'bytes': 'bytes'}
    assert record['counter_arithmetic'] == 'uint64_modulo'
    assert record['allocation_header_bytes'] == 16 and record['pool_header_bytes'] == 512
    assert record['requested_bytes_exclude_allocator_header'] is True
    assert record['size_class_payload_excludes_header'] is True
    values = record['counters']
    assert set(values) == set(COUNTERS), record
    assert all(type(n) is int and 0 <= n <= 2**64 - 1 for n in values.values()), record
    return values


def failed_stderr(image, work):
    # subprocess restores SIGPIPE's default disposition before exec. A closed
    # pipe therefore checks signal safety, rather than Python's ignored signal.
    read, write = os.pipe()
    os.close(read)
    try:
        result = subprocess.run([str(image)], cwd=work, stdout=subprocess.PIPE,
                                stderr=write, restore_signals=True, timeout=10)
        assert result.returncode == 17, result.returncode
    finally:
        os.close(write)
    with open(os.devnull, 'rb') as readonly:
        result = subprocess.run([str(image)], cwd=work, stdout=subprocess.PIPE,
                                stderr=readonly, timeout=10)
        assert result.returncode == 17, result.returncode
    result = subprocess.run([str(image)], cwd=work, stdout=subprocess.PIPE,
                            preexec_fn=lambda: os.close(2), timeout=10)
    assert result.returncode == 17, result.returncode


def image_type(path, target):
    image = path.read_bytes()
    if target == 'arm64-macos':
        assert image[:4] == b'\xcf\xfa\xed\xfe'
    else:
        assert image[:6] == b'\x7fELF\x02\x01'
        assert struct.unpack_from('<H', image, 18)[0] == (183 if target == 'arm64-linux' else 62)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='also require byte-identical unprofiled images')
    parser.add_argument('--harness-only', action='store_true', help='only verify emitted runtime helpers')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    host = 'arm64-macos' if platform.system() == 'Darwin' else (
        'arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
    with tempfile.TemporaryDirectory(prefix='dunder-runtime-profile-') as directory:
        work = Path(directory)
        emitter = work / 'emitter'
        call([compiler, 'build', FIXTURES / 'runtime-profile-emitter.ddr', '-o', emitter], cwd=ROOT)
        for target, number in TARGETS.items():
            for mode in range(4):
                output = work / f'harness-{target}-{mode}'
                call([emitter, number, mode, output, 'on'], cwd=work)
                image_type(output, target)
                if target != host:
                    continue
                result = call([output], cwd=work, status=104 if mode == 3 else 17)
                assert result.stdout == b''
                if mode == 3:
                    assert result.stderr == b''
                    continue
                values = counters(result.stderr)
                if mode == 0:
                    assert all(value == 0 for value in values.values()), values
                elif mode == 1:
                    expected = dict.fromkeys(COUNTERS, 0)
                    expected.update(allocation_count=3, requested_bytes=80, size_class_payload_bytes=96,
                                    fresh_allocations=1, reused_allocations=2, retain_calls=1,
                                    release_calls=4, record_construct_calls=1, record_construct_bytes=24,
                                    record_clone_calls=1, record_clone_bytes=24)
                    assert values == expected, values
                    failed_stderr(output, work)
                else:
                    assert values['allocation_count'] == 2**64 - 1
                    assert all(value == 0 for key, value in values.items() if key != 'allocation_count')
            unprofiled = work / f'harness-{target}-off'
            call([emitter, number, 1, unprofiled, 'off'], cwd=work)
            assert b'runtime_profile' not in unprofiled.read_bytes()
            if target == host:
                plain = call([unprofiled], cwd=work, status=17)
                assert plain.stdout == plain.stderr == b''
        print('PASS emitted runtime: zero traffic, exact event counts, unsigned formatting, '
              'traps and failed stderr; all native images build')
        if args.harness_only:
            print('CLI instrumentation coverage skipped explicitly (--harness-only)')
            return

        empty = work / 'empty.ddr'
        empty.write_text('fn main()->i32{return 17}\n')
        trap = work / 'trap.ddr'
        trap.write_text('fn main(){print(17);assert(false)}\n')
        cases = [('empty', empty), ('records', FIXTURES / 'runtime-profile-records.ddr'),
                 ('buffers', FIXTURES / 'runtime-profile-buffers.ddr'),
                 ('system', FIXTURES / 'runtime-profile-system.ddr'), ('trap', trap)]
        for name, source in cases:
            for target in TARGETS:
                output = work / f'{name}-{target}'
                plain = work / f'{name}-{target}-plain'
                call([compiler, 'build', source, '--target', target, '--runtime-profile', '-o', output], cwd=work)
                call([compiler, 'build', source, '--target', target, '-o', plain], cwd=work)
                image_type(output, target)
                if baseline:
                    previous = work / f'{name}-{target}-baseline'
                    call([baseline, 'build', source, '--target', target, '-o', previous], cwd=work)
                    assert plain.read_bytes() == previous.read_bytes(), (name, target, 'unprofiled image changed')
                if target != host:
                    continue
                status = 104 if name == 'trap' else 17
                result = call([output], cwd=work, status=status)
                reference = call([plain], cwd=work, status=status)
                assert result.stdout == reference.stdout
                assert reference.stderr == b''
                if name == 'trap':
                    assert result.stdout == b'17\n' and result.stderr == b''
                    continue
                values = counters(result.stderr)
                assert values['allocation_count'] == values['fresh_allocations'] + values['reused_allocations']
                if name == 'empty':
                    assert all(value == 0 for value in values.values()), values
                elif name == 'records':
                    for key, expected in dict(allocation_count=10, requested_bytes=160,
                                              size_class_payload_bytes=160, fresh_allocations=2,
                                              reused_allocations=8, record_construct_calls=5,
                                              record_construct_bytes=80, record_clone_calls=5,
                                              record_clone_bytes=80).items():
                        assert values[key] == expected, (key, values)
                elif name == 'buffers':
                    for key, expected in dict(allocation_count=6, requested_bytes=123,
                                              size_class_payload_bytes=176, array_clone_calls=1,
                                              array_clone_bytes=3, string_slice_calls=1,
                                              string_slice_bytes=2, copy_bytes=8,
                                              record_construct_calls=0, record_clone_calls=0).items():
                        assert values[key] == expected, (key, values)
                else:
                    assert values['record_construct_calls'] == 2 and values['record_construct_bytes'] == 56, values
                print('PASS consumer', name, flush=True)
        print('PASS ordinary stdout/exit, classified consumers and native cross-builds')


if __name__ == '__main__':
    main()
