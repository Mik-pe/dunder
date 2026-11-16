#!/usr/bin/env python3
"""Verify typed C variadic declarations against a native va_arg oracle."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MODES = ((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))


def run(command, cwd, expected=0):
    result = subprocess.run(list(map(str, command)), cwd=cwd, capture_output=True,
                            text=True, timeout=90)
    assert result.returncode == expected, (command, result.returncode,
                                            result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--cc', default='cc')
    args = parser.parse_args()
    compiler = args.compiler.resolve()
    fixture = ROOT / 'tests/fixtures/ffi'
    macos = platform.system() == 'Darwin'
    (ROOT / 'build').mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='c-variadic-', dir=ROOT / 'build') as tmp:
        work = Path(tmp)
        library = work / ('oracle.dylib' if macos else 'oracle.so')
        run([args.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC',
             '-dynamiclib' if macos else '-shared', fixture / 'variadic-oracle.c',
             '-o', library], work)
        source = work / 'variadic.ddr'
        source.write_text((fixture / 'variadic.ddr.in').read_text().replace('@LIB@', str(library)))
        runs = 0
        for index, mode in enumerate(MODES):
            output = work / f'probe-{index}'
            run([compiler, 'build', source, *mode, '-o', output], work)
            result = run([output], work)
            assert result.stdout == 'variadic ABI ok\n' and not result.stderr, result
            runs += 1

        external = '@extern("c", "abs") unsafe fn foreign(value:i32)->i32\n'
        prefix = 'capability unsafe\n'
        cases = [
            ('@c_variadic(-1) ' + external, 'nonnegative fixed parameter count'),
            ('@c_variadic(2) ' + external, 'invalid C variadic fixed parameter count'),
            ('@c_variadic(999999) ' + external, 'invalid C variadic fixed parameter count'),
            ('@c_variadic("one") ' + external, 'nonnegative fixed parameter count'),
            ('@c_variadic() ' + external, 'nonnegative fixed parameter count'),
            ('@c_variadic(1) @c_variadic(1) ' + external, 'duplicate @c_variadic'),
            ('@c_variadic(1) unsafe fn foreign(value:i32)->i32 { value }\n', '@c_variadic requires an @extern'),
        ]
        for ty in ('f32', 'u8', 'u16'):
            cases.append((f'@c_variadic(1) @extern("c","f") unsafe fn foreign(fixed:i32,value:{ty})\n',
                          'require promoted scalar types'))
        cases.append(('@c_layout struct Pair { a:f64,b:f64 }\n'
                      '@c_variadic(1) @extern("c","f") unsafe fn foreign(fixed:i32,value:Pair)\n',
                      'require promoted scalar types'))
        rejected = 0
        for index, (declaration, message) in enumerate(cases):
            bad = work / f'invalid-{index}.ddr'
            bad.write_text(prefix + declaration + 'fn main() {}\n')
            result = run([compiler, 'check', bad], work, 1)
            assert message in result.stderr, (index, result.stderr)
            rejected += 1

        for body in ('f:extern fn(i32)->i32=foreign',
                     'first:=foreign;f:extern fn(i32)->i32=first'):
            bad = work / f'invalid-{rejected}.ddr'
            bad.write_text(prefix + '@c_variadic(1) ' + external +
                           'fn main() { unsafe { ' + body + ' } }\n')
            result = run([compiler, 'check', bad], work, 1)
            assert 'type mismatch' in result.stderr, result.stderr
            rejected += 1

        boundary = work / 'invalid-boundary.ddr'
        boundary.write_text(prefix +
                            '@c_variadic(1) @extern("c","f") unsafe fn first(a:i32,b:i32)->i32\n'
                            '@c_variadic(2) @extern("c","f") unsafe fn second(a:i32,b:i32)->i32\n'
                            'fn main() { unsafe { mut call:=first;call=second } }\n')
        result = run([compiler, 'check', boundary], work, 1)
        assert 'type mismatch' in result.stderr, result.stderr
        rejected += 1

        unsupported = work / 'unsupported-wasm.ddr'
        unsupported.write_text(prefix + '@c_variadic(1) ' + external + 'fn main() {}\n')
        result = run([compiler, 'check', unsupported, '--target', 'wasm32'], work, 1)
        assert 'require a native target' in result.stderr, result.stderr
        print(json.dumps({'native_executions': runs, 'rejected': rejected,
                          'wasm_rejected': True, 'host': platform.system() + '/' + platform.machine()}))


if __name__ == '__main__':
    main()
