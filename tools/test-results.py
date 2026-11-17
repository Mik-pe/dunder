#!/usr/bin/env python3
"""Check Result specializations, propagation, resource transfer and diagnostics."""
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
OUTPUT = '11\n50\n50\n1\n2\n51\n3\n4\n52\n53\n5\n42\n'
WASM = """
const fs = require('node:fs');
const image = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const instance = new WebAssembly.Instance(image, {dunder: {print_i32: console.log, print_text: () => {}}});
instance.exports.main();
"""

INVALID = (
    ('fn main() { x := 3? }', "'?' requires a Result operand"),
    ('fn value() -> Result<i32, string> { .Ok(1) } fn main() { x := value()? }',
     "'?' requires a Result operand and a function returning Result"),
    ('fn value() -> Result<i32, string> { .Err("no") } '
     'fn other() -> Result<i32, i32> { .Ok(value()?) } fn main() { _ := other() }',
     'same error type'),
    ('fn other(x: Result<string, i32>) -> Result<string, i32> { .Ok(x?) } fn main() { _ := other(.Ok("v")) }',
     'cannot move out of a borrowed value'),
    ('fn other(move x: Result<string, i32>) -> Result<i32, i32> '
     '{ s := x?; match x { .Ok(_) => { assert(false) }, .Err(_) => { assert(false) } }; .Ok(s.len) } '
     'fn main() { _ := other(.Ok("x")) }', 'use of moved value'),
    ('fn unit() -> Result<(), i32> { .Ok } fn deferred() -> Result<(), i32> { defer { unit()? }; .Ok } '
     'fn main() { _ := deferred() }',
     'not allowed inside defer'),
    ('fn main() { x: Result<&i32, string> = .Err("no") }', 'Result type arguments must be storable values'),
    ('fn main() { x: Result<i32, bool> = .Err("no") }', 'type mismatch'),
    ('fn main() { x: Result<i32, bool> = .Ok(1); match x { .Ok(_) => {} } }', 'non-exhaustive'),
)


def invoke(compiler, work, *args, status=0):
    result = subprocess.run([compiler, *args], cwd=work, capture_output=True, text=True, timeout=120)
    assert result.returncode == status, (args, result.returncode, result.stdout, result.stderr)
    return result


def reordered_specializations(compiler, work):
    source = work / 'reordered.ddr'
    ordered = [
        'words := Result<[]string, i32>.Ok(["word"])',
        'numbers := Result<[]i32, i32>.Ok([1])',
    ]
    suffix = ('; print(19) }\n'
              'fn second() { value := Result<[]i32, i32>.Ok([1]); print(23) }\n'
              'fn main() { first(); second() }\n')
    image = work / 'reordered'
    for order in (ordered, reversed(ordered)):
        source.write_text('fn first() { ' + '; '.join(order) + suffix)
        invoke(compiler, work, 'build', source, '--incremental', '--inline-opt=off', '-o', image)
        assert invoke(image, work).stdout == '19\n23\n'


def main():
    compiler = Path(sys.argv[1]).resolve(strict=True)
    machine = platform.machine()
    host = 'arm64-macos' if platform.system() == 'Darwin' else (
        'x86_64-linux' if machine == 'x86_64' else 'arm64-linux')
    source = ROOT / 'tests/fixtures/results.ddr'
    with tempfile.TemporaryDirectory(prefix='dunder-results-') as directory:
        work = Path(directory)
        for target in TARGETS:
            for mode in (['--jobs', '4'] if target == host else ['--jobs', '1'],
                         ['--jobs', '1', '--mir=off', '--scalar-opt=off', '--ownership-opt=off']):
                image = work / target
                invoke(compiler, work, 'build', source, '--target', target, *mode, '-o', image)
                if target == host:
                    assert invoke(image, work).stdout == OUTPUT
                elif target == 'wasm32':
                    assert invoke('node', work, '-e', WASM, image).stdout == OUTPUT
        local = work / 'incremental.ddr'
        shutil.copyfile(source, local)
        for _ in range(2):
            assert invoke(compiler, work, 'run', local, '--incremental', '--jobs', '4').stdout == OUTPUT
        reordered_specializations(compiler, work)

        for index, (body, message) in enumerate(INVALID):
            invalid = work / f'invalid-{index}.ddr'
            invalid.write_text(body + '\n')
            diagnostic = json.loads(invoke(compiler, work, 'check', invalid, '--agent', '--jobs', '4', status=1).stdout)
            assert diagnostic['kind'] == 'error' and message in diagnostic['message'], (body, diagnostic)
            assert diagnostic['path'] == str(invalid), diagnostic
            assert 0 <= diagnostic['start'] < diagnostic['end'] <= invalid.stat().st_size, diagnostic
            image = work / 'previous-image'
            image.write_bytes(b'previous image')
            build = invoke(compiler, work, 'build', invalid, '-o', image, status=1)
            assert message in build.stderr and image.read_bytes() == b'previous image', build
    print('PASS Result propagation on native and Wasm, four target builds, cached builds and diagnostics')


if __name__ == '__main__':
    main()
