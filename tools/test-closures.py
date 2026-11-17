#!/usr/bin/env python3
"""Check closure lifetimes, indirect calls, module scopes and source diagnostics."""
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
VALUES = ROOT / 'tests/fixtures/closures/values.ddr'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
HOST = {('Darwin', 'arm64'): 'arm64-macos', ('Linux', 'aarch64'): 'arm64-linux',
        ('Linux', 'arm64'): 'arm64-linux', ('Linux', 'x86_64'): 'x86_64-linux'}
INVALID = (
    ('n := 3; f := fn() -> i32 { n }', "unknown local 'n'"),
    ('n := 3; f := fn[n: bool]() -> bool { n }', 'type mismatch'),
    ('n := 3; f := fn[n]() -> i32 { n }', 'expected a closure capture'),
    ('n := 3; f := fn[n: i32, n: i32]() -> i32 { n }', 'duplicate closure capture'),
    ('n := 3; f := fn[n: i32](n: i32) -> i32 { n }', 'duplicate local name'),
    ('n := 3; f := fn[n: &i32]() -> i32 { n }', 'cannot store a borrowed reference'),
    ('mut n := 3; f := fn[n: i32]() { n = 4 }', 'mutable stored value'),
    ('s := "a"; f := fn[s: string]() -> i32 { s.len }; print(s)', 'use of moved value'),
    ('n := 3; f := fn[n: i32]() -> &i32 { &n }', 'shared input parameter'),
    ('n := 3; f: fn(bool) -> i32 = fn[n: i32](x: i32) -> i32 { x+n }', 'type mismatch'),
    ('f := fn(x: i32) -> i32 { true }', 'type mismatch'),
)

WASM = """
const fs = require('node:fs');
const moduleImage = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const runtime = new WebAssembly.Instance(moduleImage, {dunder: {print_i32: console.log}});
runtime.exports.main();
"""


def run(command, cwd, expected=0):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=90)
    assert result.returncode == expected, (command, result.stdout, result.stderr)
    return result


def main():
    compiler = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='dunder-closures-') as directory:
        work = Path(directory)
        image = work / 'values'
        for target in TARGETS:
            for flags in (['--jobs', '4'], ['--jobs', '1', '--mir=off', '--scalar-opt=off']):
                run([compiler, 'build', VALUES, '--target', target, *flags, '-o', image], work)
                if target == HOST[platform.system(), platform.machine()]:
                    assert run([image], work).stdout == '42\n'
                elif target == 'wasm32':
                    assert run(['node', '-e', WASM, image], work).stdout == '42\n'

        run([compiler, 'build', VALUES, '--runtime-profile', '-o', image], work)
        profiled = run([image], work)
        counts = json.loads(profiled.stderr)['counters']
        assert profiled.stdout == '42\n' and counts['fresh_allocations'] < 64, counts
        assert counts['reused_allocations'] >= 256, counts

        # Both CLI entry points must reject the same invalid expression.
        source = work / 'invalid.ddr'
        for body, message in INVALID:
            source.write_text('fn main() { ' + body + ' }\n')
            error = json.loads(run([compiler, 'check', source, '--agent', '--jobs', '4'], work, 1).stdout)
            assert error['kind'] == 'error' and message in error['message'], (body, error)
            assert error['path'] == str(source) and 0 <= error['start'] < error['end'] <= source.stat().st_size, error
            failed = run([compiler, 'build', source, '--jobs', '4', '-o', image], work, 1)
            assert message in failed.stderr, (body, failed.stderr)

        suite = work / 'suite.ddr'
        suite.write_text('@test fn literals() { n := 40; f := fn[n: i32](x: i32) -> i32 { x + n }; assert(f(2) == 42) }\n')
        assert 'literals' in run([compiler, 'test', suite, '-o', work / 'suite'], work).stdout
    print('Closure calls, ownership, target builds and diagnostics passed.')


if __name__ == '__main__':
    main()
