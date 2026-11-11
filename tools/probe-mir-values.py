#!/usr/bin/env python3
"""Ablate MIR values on the unchanged numeric/text runtime benchmark sources.

One fresh off/on build per source/target, finite correctness samples, and actual
--timings=json counters. No timing series or per-loop/disassembly claims. Exact
Wasm frontend rejections are preserved as BLOCKED coverage, never rewritten into
an adapter program. Exit2 means native checks passed but Wasm remains blocked.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
VALUE_KEYS = ('values_optimized', 'value_rewrites', 'value_deleted', 'value_immediates',
              'value_analysis_units', 'value_budget_refusals')
NATIVE_ONLY = re.compile(r'native system calls need a native target|requires a native target', re.I)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    return sha(Path(path).read_bytes())


def invoke(command, work, timeout=60):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=timeout)
    return result


def checked(command, work):
    result = invoke(command, work)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return result


def numeric(count, seed):
    total, value = 0, seed
    for _ in range(count):
        number = value * 1664525 + 1013904223
        quotient = abs(number) // 2147483647
        if number < 0:
            quotient = -quotient
        value = number - quotient * 2147483647
        total += value
    return str(total) + '\n'


def text(count, value):
    words, inside = 0, False
    for scalar in value:
        if scalar in ' \n\t':
            inside = False
        elif not inside:
            words += 1
            inside = True
    return str(count * (words * 1000000 + sum(map(ord, value)))) + '\n'


def sources():
    return {
        'numeric': (ROOT / 'benchmarks/runtime/numeric.ddr', [
            (['0', '9223372036854775807'], numeric(0, 9223372036854775807)),
            (['1', '42'], numeric(1, 42)),
            (['13', '-2147483646'], numeric(13, -2147483646)),
            (['257', '2147483646'], numeric(257, 2147483646)),
        ]),
        'text': (ROOT / 'benchmarks/runtime/text.ddr', [
            (['3', ''], text(3, '')),
            (['3', 'Hej  åäö 😀\n世界\tabc'], text(3, 'Hej  åäö 😀\n世界\tabc')),
            (['2', '\talpha\n\nβeta  😀delta\t'], text(2, '\talpha\n\nβeta  😀delta\t')),
            (['7', 'a\u00a0b'], text(7, 'a\u00a0b')),
            (['0', '🥨\tzero trip'], text(0, '🥨\tzero trip')),
        ]),
    }


def uleb(data, offset):
    value = 0
    for part in range(5):
        assert offset < len(data), 'truncated Wasm length'
        byte = data[offset]; offset += 1
        value |= (byte & 127) << (part * 7)
        if byte < 128:
            return value, offset
    raise AssertionError('invalid Wasm length')


def regions(data):
    """Container regions only: executable bytes may include embedded data."""
    found = []
    if data[:4] == b'\xcf\xfa\xed\xfe':
        commands, limit = struct.unpack_from('<II', data, 16)
        offset = 32
        assert offset + limit <= len(data)
        for _ in range(commands):
            kind, size = struct.unpack_from('<II', data, offset)
            assert size >= 8 and offset + size <= 32 + limit
            if kind == 25:
                sections = struct.unpack_from('<I', data, offset + 64)[0]
                assert 72 + sections * 80 <= size
                for i in range(sections):
                    section = offset + 72 + i * 80
                    name = data[section:section + 16].split(b'\0')[0]
                    if name == b'__text':
                        count = struct.unpack_from('<Q', data, section + 40)[0]
                        start = struct.unpack_from('<I', data, section + 48)[0]
                        found.append(('macho.__text', start, count))
            offset += size
    elif data[:6] == b'\x7fELF\x02\x01':
        offset = struct.unpack_from('<Q', data, 32)[0]
        width, count = struct.unpack_from('<HH', data, 54)
        assert width >= 56 and offset + width * count <= len(data)
        for i in range(count):
            start = offset + i * width
            kind, flags, position = struct.unpack_from('<IIQ', data, start)
            length = struct.unpack_from('<Q', data, start + 32)[0]
            if kind == 1 and flags & 1:
                found.append((f'elf.executable_load_{i}', position, length))
    elif data[:8] == b'\0asm\x01\0\0\0':
        offset = 8
        while offset < len(data):
            section = data[offset]; length, start = uleb(data, offset + 1)
            if section == 10:
                found.append(('wasm.code_section', start, length))
            assert start + length <= len(data)
            offset = start + length
    result = []
    for name, start, length in found:
        assert start >= 0 and length >= 0 and start + length <= len(data)
        result.append({'name': name, 'offset': start, 'bytes': length,
                       'sha256': sha(data[start:start + length])})
    return result


def difference(before, after):
    prefix = 0
    while prefix < min(len(before), len(after)) and before[prefix] == after[prefix]:
        prefix += 1
    return {'byte_identical': before == after, 'bytes_delta': len(after) - len(before),
            'first_differing_offset': None if before == after else prefix,
            'aligned_differing_bytes': sum(a != b for a, b in zip(before, after)) + abs(len(after) - len(before))}


def build(compiler, source, target, mode, image, work):
    command = [compiler, 'build', source, '--target', target, '--inline-opt=on',
               '--mir=on', '--mir-opt=on', f'--mir-values={mode}', '--timings=json', '-o', image]
    result = invoke(command, work)
    record = {'command': list(map(str, command)), 'returncode': result.returncode,
              'stdout': result.stdout, 'stderr': result.stderr}
    if result.returncode:
        assert target == 'wasm32' and NATIVE_ONLY.search(result.stderr), record
        record.update(status='blocked_native_argv_frontend', executed=False)
        return record
    reports = [json.loads(line) for line in result.stderr.splitlines() if line.startswith('{')]
    assert len(reports) == 1 and reports[0]['kind'] == 'timings', record
    report = reports[0]; backend = report['mir']
    assert report['target'] == target and report['inlining']['enabled'] is True
    assert backend['enabled'] is True and backend['optimization_enabled'] is True
    assert backend['values_enabled'] is (mode == 'on')
    assert all(isinstance(backend[key], int) and backend[key] >= 0 for key in VALUE_KEYS), backend
    assert report['functions']['reused'] == 0 and backend['functions'] > 0
    if mode == 'off':
        assert all(backend[key] == 0 for key in VALUE_KEYS), backend
    data = image.read_bytes()
    record.update(status='built', timings=report, image_sha256=sha(data), image_bytes=len(data), regions=regions(data))
    return record


def self_test():
    assert numeric(0, 9223372036854775807) == '0\n'
    assert numeric(1, -2147483646) == '-1131914899\n'
    assert text(3, '') == '0\n' and text(1, 'a\u00a0b') == '1000355\n'
    assert difference(b'abc', b'abc')['byte_identical']
    assert difference(b'abc', b'abde')['aligned_differing_bytes'] == 2
    assert regions(b'\0asm\x01\0\0\0\x0a\x01\x00') == [
        {'name': 'wasm.code_section', 'offset': 10, 'bytes': 1, 'sha256': sha(b'\0')}]
    print('PASS source references and safe container comparison')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, nargs='?')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--wasm-runner', type=Path, help='optional argv-capable runner: runner IMAGE ARG...')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test(); return 0
    assert args.compiler and args.output, 'compiler and --output are required'
    compiler = args.compiler.resolve(strict=True); compiler_hash = file_sha(compiler)
    host = 'arm64-macos' if platform.system() == 'Darwin' else ('arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
    evidence = {'scope': 'unchanged_benchmark_source_values_ablation', 'timing_series': False,
                'per_loop_dump': False, 'compiler': str(compiler), 'compiler_sha256': compiler_hash,
                'native_target': host, 'inline': 'on', 'incremental': False, 'benchmarks': {}, 'blocked': []}
    with tempfile.TemporaryDirectory(prefix='dunder-source-values-') as directory:
        work = Path(directory)
        for name, (source, samples) in sources().items():
            source_hash = file_sha(source); item = {'source': str(source), 'source_sha256': source_hash, 'targets': {}}
            for target in (host, 'wasm32'):
                rows = {}; images = {}
                for mode in ('off', 'on'):
                    image = work / f'{name}-{target}-{mode}'
                    row = build(compiler, source, target, mode, image, work); rows[mode] = row
                    assert file_sha(source) == source_hash, 'benchmark source changed'
                    if row['status'] != 'built':
                        evidence['blocked'].append(f'{name}/{target}/{mode}: {row["status"]}'); continue
                    images[mode] = image.read_bytes(); executions = []
                    for argv, expected in samples:
                        if target == 'wasm32':
                            if not args.wasm_runner:
                                row['status'] = 'blocked_missing_argv_host_runner'
                                evidence['blocked'].append(f'{name}/{target}/{mode}: {row["status"]}'); break
                            command = [args.wasm_runner.resolve(strict=True), image, *argv]
                        else:
                            command = [image, *argv]
                        result = checked(command, work)
                        assert result.stdout == expected, (name, target, mode, argv, result.stdout, expected)
                        executions.append({'argv': argv, 'expected': expected, 'stdout': result.stdout, 'returncode': result.returncode})
                    row.update(executions=executions, executed=len(executions) == len(samples))
                if len(images) == 2:
                    comparison = difference(images['off'], images['on'])
                    before = rows['off']['timings']['mir']; after = rows['on']['timings']['mir']
                    comparison['mir_counter_delta'] = {key: after[key] - before[key] for key in before
                                                       if isinstance(before[key], int) and not isinstance(before[key], bool)
                                                       and isinstance(after.get(key), int)}
                    comparison['region_bytes_delta'] = {region['name']: region['bytes'] - old['bytes']
                                                        for region in rows['on']['regions'] for old in rows['off']['regions']
                                                        if old['name'] == region['name']}
                    if after['value_rewrites'] + after['value_deleted'] > 0:
                        assert not comparison['byte_identical'], 'reported value transformations did not change the image'
                else:
                    assert rows['off']['status'] == rows['on']['status'], rows
                    comparison = {'byte_identical': None, 'status': 'no_images',
                                  'diagnostic_identical': rows['off']['stderr'] == rows['on']['stderr']}
                    assert comparison['diagnostic_identical'], comparison
                item['targets'][target] = {'variants': rows, 'comparison': comparison}
            evidence['benchmarks'][name] = item
    assert file_sha(compiler) == compiler_hash, 'compiler changed during probe'
    evidence['status'] = 'partial_blocked_wasm' if evidence['blocked'] else 'verified_native_and_wasm'
    output = args.output.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    for name, item in evidence['benchmarks'].items():
        native = item['targets'][host]; counts = native['variants']['on']['timings']['mir']
        print(f'{name}: native outputs verified; values rewrites={counts["value_rewrites"]} '
              f'deleted={counts["value_deleted"]} immediate={counts["value_immediates"]}; '
              f'byte_identical={native["comparison"]["byte_identical"]}')
    for blocked in evidence['blocked']:
        print('BLOCKED ' + blocked)
    print(output)
    return 2 if evidence['blocked'] else 0


if __name__ == '__main__':
    sys.exit(main())
