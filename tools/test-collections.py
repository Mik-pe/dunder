#!/usr/bin/env python3
"""Verify static collection specialization, ownership and target behavior."""
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
OUTPUT = '1\n12\n2\n11\n13\n14\n6\n14\n42\n'
WASM = """
const fs = require('node:fs');
const image = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const instance = new WebAssembly.Instance(image, {dunder: {print_i32: console.log, print_text: () => {}}});
instance.exports.main();
"""
INVALID = (
    ('fn main() { _ := Map<i32>::new() }', 'Map expects a key and a value type'),
    ('fn main() { _ := Map<i32, i32, i32>::new() }', 'Map expects a key and a value type'),
    ('fn main() { _ := Set<i32, i64>::new() }', 'Set expects one element type'),
    ('fn main() { _ := Map<string, &i32>::new() }', 'keys and values must be storable'),
    ('fn main() { _ := Map<string, ()>::new() }', 'keys and values must be storable'),
    ('struct Key { id: i32 } fn main() { _ := Set<Key>::new() }', "unknown method 'key_hash'"),
    ('struct Key { id: i32 } impl Key { pub fn key_hash(&self) -> u64 { 0 } } '
     'fn main() { _ := Map<Key, i32>::new() }', "unknown method 'key_eq'"),
    ('fn main() { t := Map<i32, bool>::new(); print(t.entries.len) }', 'an opaque struct'),
    ('fn main() { mut t := Map<i32, string>::new(); t.put(1, "old"); '
     'value := t.at(1); t.put(1, "new"); print(value) }', 'while it is borrowed'),
    ('@resource struct Handle { fd: i32 } fn main() { t := Map<i32, Handle>::new(); '
     '_ := t.clone() }', 'resource values cannot be cloned'),
    ('@resource struct Handle { fd: i32 } fn main() { mut t := Map<i32, Handle>::new(); '
     'h := Handle {fd:1}; t.put(2, h); print(h.fd) }', 'move'),
)


def run(directory, command, status=0):
    result = subprocess.run(command, cwd=directory, capture_output=True, text=True, timeout=120)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def main():
    compiler = Path(sys.argv[1]).resolve(strict=True)
    host = 'arm64-macos' if platform.system() == 'Darwin' else (
        'x86_64-linux' if platform.machine() == 'x86_64' else 'arm64-linux')
    source = ROOT / 'tests/fixtures/collections.ddr'
    with tempfile.TemporaryDirectory(prefix='dunder-collections-') as directory:
        work = Path(directory)
        for target in TARGETS:
            for flags in (['--jobs', '4' if target == host else '1'],
                          ['--jobs', '1', '--mir=off', '--scalar-opt=off', '--ownership-opt=off']):
                image = work / target
                run(work, [compiler, 'build', source, '--target', target, *flags, '-o', image])
                if target == host:
                    assert run(work, [image]).stdout == OUTPUT
                elif target == 'wasm32':
                    assert run(work, ['node', '-e', WASM, image]).stdout == OUTPUT
        cached = work / 'cached.ddr'
        shutil.copyfile(source, cached)
        for _ in range(2):
            result = run(work, [compiler, 'run', cached, '--jobs', '4', '--incremental'])
            assert result.stdout == OUTPUT
        for index, (program, error) in enumerate(INVALID):
            bad = work / f'invalid-{index}.ddr'
            bad.write_text(program + '\n')
            checked = run(work, [compiler, 'check', bad, '--jobs', '4', '--agent'], 1)
            diagnostic = json.loads(checked.stdout)
            assert diagnostic['kind'] == 'error' and error in diagnostic['message'], diagnostic
            assert diagnostic['path'] == str(bad), diagnostic
            assert 0 <= diagnostic['start'] < diagnostic['end'] <= bad.stat().st_size, diagnostic
            image = work / 'existing'
            image.write_bytes(b'previous image')
            rejected = run(work, [compiler, 'build', bad, '-o', image], 1)
            assert error in rejected.stderr and image.read_bytes() == b'previous image', rejected
    print('PASS specialized collections across four targets, native and Wasm cleanup, and diagnostics')


if __name__ == '__main__':
    main()
