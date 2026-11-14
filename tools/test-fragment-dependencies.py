#!/usr/bin/env python3
"""Validate dependency/growth cache payloads, atomic rejection and code identity.

Builds one consumer fixture, creates actual cold/hot native-vocabulary images
for all four targets, executes the host and Wasm, and signs deliberately
malformed payloads independently so the format checks themselves are exercised.
No compiler hooks, cache policy or performance measurements are involved.
"""
import argparse
import hashlib
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
import wasm_host

ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH': '/no-external-tools'}


def run(command, work, expected=b'42\n'):
    result = subprocess.run([str(x) for x in command], cwd=work, env=ENV,
                            capture_output=True, timeout=120)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    if expected is not None:
        assert result.stdout == expected, (command, result.stdout, expected)
    return result


def payload(data):
    assert len(data) >= 184 and len(data[128:]) % 8 == 0
    return [int(data[i:i+8], 16) for i in range(128, len(data), 8)]


def signed(key, numbers=None, raw=None):
    if raw is None:
        raw = ''.join(f'{n:08x}' for n in numbers).encode()
    return key + hashlib.sha256(key + raw).hexdigest().encode() + raw


def malformed(numbers):
    word_count, label_count, reloc_count, dependency_count = numbers[2:6]
    label_at = 7 + word_count
    reloc_at = label_at + label_count
    deps_at = reloc_at + reloc_count * 3
    assert label_count >= 3 and reloc_count >= 3 and dependency_count == 3
    changes = {
        'magic': (0, 0), 'version': (1, 2), 'zero-words': (2, 0),
        'huge-words': (2, 0xffffffff), 'huge-labels': (3, 0xffffffff),
        'huge-relocs': (4, 0xffffffff), 'too-many-deps': (5, 257),
        'bad-dep-count': (5, 2), 'charge-cap': (6, 4097),
        'negative-charge': (6, 0xffffffff), 'bad-label': (label_at, word_count + 1),
        'reloc-offset': (reloc_at, word_count), 'reloc-kind': (reloc_at + 2, 21),
        'conditional-width': (reloc_at, word_count - 1),
        'private-target': (reloc_at + 1, label_count),
        'unbound-private-target': (reloc_at + 1, label_count - 1),
        'global-target': (reloc_at + 1, 0x80000003),
        'huge-global-target': (reloc_at + 1, 0xffffffff),
        'duplicate-dep': (deps_at + 1, numbers[deps_at]),
        'unsorted-dep': (deps_at + 1, 1),
        'unknown-dep': (deps_at + 2, 16),
        'negative-dep': (deps_at, 0xffffffff),
    }
    for name, (index, value) in changes.items():
        changed = list(numbers); changed[index] = value
        yield name, changed
    yield 'truncated-word', numbers[:-1]
    yield 'extra-word', numbers + [0]


def malformed_native3(numbers):
    words, labels, relocs = numbers[:3]
    label_at = 3 + words
    reloc_at = label_at + labels
    for name, index, value in (
            ('zero-words', 0, 0), ('huge-words', 0, 0xffffffff),
            ('huge-labels', 1, 0xffffffff), ('huge-relocs', 2, 0xffffffff),
            ('bad-label', label_at, words + 1), ('unset-label', label_at, 0xffffffff),
            ('bad-at', reloc_at, words), ('negative-at', reloc_at, 0xffffffff),
            ('kind', reloc_at + 2, 21), ('conditional-width', reloc_at, words - 1),
            ('private-target', reloc_at + 1, labels),
            ('unbound-private', reloc_at + 1, labels - 1),
            ('global-target', reloc_at + 1, 0x80000003)):
        changed = list(numbers); changed[index] = value
        yield name, changed
    yield 'truncated', numbers[:-1]
    yield 'extra', numbers + [0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--node', default=None)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    compiler_hash = hashlib.sha256(compiler.read_bytes()).hexdigest()
    node = wasm_host.js_runtime(args.node)
    if platform.system() == 'Darwin' and platform.machine() == 'arm64':
        host = 0
    elif platform.system() == 'Linux' and platform.machine() in ('arm64', 'aarch64', 'x86_64'):
        host = 1 if platform.machine() in ('arm64', 'aarch64') else 2
    else:
        parser.error('execution requires ARM64 macOS or ARM64/x86-64 Linux')
    images = executions = rejections = 0
    with tempfile.TemporaryDirectory(prefix='dunder-fragment-deps-') as directory:
        work = Path(directory).resolve()
        fixture = work / 'fixture'
        run([compiler, 'build', ROOT / 'tests/fixtures/fragment-dependencies.ddr', '-o', fixture], work, None)
        run([fixture, 'unit'], work)
        for target in range(4):
            for selection, limit, budget in (('normal', 16, 23), ('builtin', 0, 23), ('full', 256, 4096)):
                cache = work / f'{target}-{selection}.fn'
                cold = work / f'{target}-{selection}-cold'
                run([fixture, 'generate', cache, target, cold, 0, selection], work)
                new_data = cache.read_bytes(); numbers = payload(new_data); key = new_data[:64]
                assert numbers[:2] == [0x44554644, 1]
                assert numbers[5] == {'normal': 3, 'builtin': 0, 'full': 256}[selection]
                assert numbers[6] == budget
                # Independent native3 encoder: new metadata must not change the
                # unchanged old save() byte format or the actual code section.
                count = numbers[2] + numbers[3] + 3 * numbers[4]
                old_numbers = numbers[2:5] + numbers[7:7+count]
                legacy = Path(str(cache) + '.native3')
                assert legacy.read_bytes() == signed(key, old_numbers), 'native3 bytes changed'
                for padding in (0, 1):
                    direct = cold
                    if padding:
                        direct = work / f'{target}-{selection}-rebased-cold'
                        scratch_cache = work / f'{target}-{selection}-rebased.fn'
                        run([fixture, 'generate', scratch_cache, target, direct, padding, selection], work)
                    for mode, fragment, expected_deps in (
                            ('restore', cache, selection), ('restore-old', legacy, selection),
                            ('restore-cached', cache, selection), ('restore-cached', legacy, 'empty')):
                        hot = work / f'{target}-{selection}-{padding}-{mode}-{expected_deps}'
                        allowed, available = (0, 0) if expected_deps == 'empty' else (limit, budget)
                        run([fixture, mode, fragment, target, hot, padding, allowed, available, 'yes', expected_deps], work)
                        assert hot.read_bytes() == direct.read_bytes(), (target, selection, padding, mode, 'cold/hot image mismatch')
                        images += 1
                        if target in (host, 3):
                            command = [hot] if target == host else [node, ROOT / 'tools/runtime-contract-wasm.cjs', hot]
                            run(command, work, b'')
                            executions += 1
                if selection == 'normal':
                    run([fixture, 'save-invalid', work / f'invalid-save-{target}', target, work / 'unused', 0], work)
                    for label, entries in (('native3-minimum', [1, 0, 0, 3596551104]),
                                           ('dufd-minimum', [0x44554644, 1, 1, 0, 0, 0, 0, 3596551104])):
                        minimal = work / f'{target}-{label}.fn'; minimal.write_bytes(signed(key, entries))
                        run([fixture, 'restore-minimum', minimal, target, work / 'unused', 0, 0, 0, 'yes', 'empty'], work)
                    # The old and new readers must both fail closed on the
                    # other's format, preserving Code and output metadata.
                    for mode, fragment in (('restore', legacy), ('restore-old', cache)):
                        image = work / 'unchanged-image'; image.write_bytes(b'previous image')
                        run([fixture, mode, fragment, target, image, 0, limit, budget, 'no'], work, b'0\n')
                        assert image.read_bytes() == b'previous image'
                        rejections += 1
                    for guard in ('marked', 'checking', 'entry', 'globals', 'reloc-arrays'):
                        for mode, fragment in (('restore', cache), ('restore-cached', cache), ('restore-cached', legacy)):
                            run([fixture, mode, fragment, target, work / 'unused', 0, limit, budget,
                                 'no', selection, guard], work, b'0\n')
                            rejections += 1
                    for label, entries in malformed(numbers):
                        bad = work / f'{target}-{label}.fn'; bad.write_bytes(signed(key, entries))
                        for mode in ('restore', 'restore-cached'):
                            run([fixture, mode, bad, target, work / 'unused', 0, limit, 65536, 'no'], work, b'0\n')
                            rejections += 1
                    for label, entries in malformed_native3(old_numbers):
                        bad = work / f'{target}-native3-{label}.fn'; bad.write_bytes(signed(key, entries))
                        for mode in ('restore-old', 'restore-cached'):
                            run([fixture, mode, bad, target, work / 'unused', 0, 0, 0, 'no'], work, b'0\n')
                            rejections += 1
                    for label, raw in (('non-hex', b'g' + new_data[129:]),
                                       ('unaligned', new_data[128:] + b'0'),
                                       ('header-only', new_data[128:184])):
                        bad = work / f'{target}-{label}.fn'; bad.write_bytes(signed(key, raw=raw))
                        for mode in ('restore', 'restore-cached'):
                            run([fixture, mode, bad, target, work / 'unused', 0, limit, budget, 'no'], work, b'0\n')
                            rejections += 1
                    for label, raw in (('bad-signature', key + b'0'*64 + new_data[128:]),
                                       ('wrong-key', b'0'*64 + new_data[64:]),
                                       ('short-file', new_data[:128]), ('empty-file', b'')):
                        bad = work / f'{target}-{label}.fn'; bad.write_bytes(raw)
                        for mode in ('restore', 'restore-cached'):
                            run([fixture, mode, bad, target, work / 'unused', 0, limit, budget, 'no'], work, b'0\n')
                            rejections += 1
                for bad_limit, bad_budget in ((limit, budget-1), (limit, -1), (-1, 65536)):
                    for mode in ('restore', 'restore-cached'):
                        run([fixture, mode, cache, target, work / 'unused', 0, bad_limit, bad_budget, 'no'], work, b'0\n')
                        rejections += 1
                for bad_limit, bad_budget in ((-1, 0), (0, -1)):
                    run([fixture, 'restore-cached', legacy, target, work / 'unused', 0, bad_limit, bad_budget, 'no'], work, b'0\n')
                    rejections += 1
                if selection != 'builtin':
                    bad_limit = 9 if selection == 'normal' else 255
                    for mode in ('restore', 'restore-cached'):
                        run([fixture, mode, cache, target, work / 'unused', 0, bad_limit, 65536, 'no'], work, b'0\n')
                        rejections += 1
                print('PASS', f'target={target}', selection, 'metadata, native3 bytes, cold/hot rebasing', flush=True)
        oversized = work / 'oversized.fn'
        with oversized.open('wb') as file:
            file.truncate(67108865)
        for mode in ('restore', 'restore-cached'):
            run([fixture, mode, oversized, host, work / 'unused', 0, 16, 65536, 'no'], work, b'0\n')
            rejections += 1
    assert hashlib.sha256(compiler.read_bytes()).hexdigest() == compiler_hash, 'compiler changed during test'
    print(f'{images} byte-identical cold/hot images; {executions} host/Wasm executions; '
          f'{rejections} atomic restore rejections; record/charge/reset and invalid-save checks passed')


if __name__ == '__main__':
    main()
