#!/usr/bin/env python3
"""Verify interpolation through native CLI, caches, diagnostics and source/token APIs."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
FIXTURES = ROOT / 'tests/fixtures/interpolation'


def call(program, args, cwd, status=0, expected=None):
    result = subprocess.run([str(program), *map(str, args)], cwd=cwd,
                            capture_output=True, text=True, timeout=90)
    if result.returncode != status or (expected is not None and result.stdout != expected):
        raise AssertionError(f'{args}: exit {result.returncode}, expected {status}\n'
                             f'{result.stdout!r}\n{result.stderr}')
    return result


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if system == 'Linux' and machine == 'x86_64':
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host {system}/{machine}')


def assert_image(image, target):
    data = image.read_bytes()
    if target == 'arm64-macos':
        assert data[:4] == b'\xcf\xfa\xed\xfe', image
    else:
        assert data[:4] == b'\x7fELF', image
        assert int.from_bytes(data[18:20], 'little') == (183 if target == 'arm64-linux' else 62)


def diagnostic_contracts(compiler, work):
    invalid = (
        ('empty', 'fn main(){print(f"{}")}', 'interpol'),
        ('comment-empty', 'fn main(){print(f"{/* only a comment */}")}', 'interpol'),
        ('unclosed-hole', 'fn main(){print(f"{42")}', ''),
        ('closing-brace', 'fn main(){print(f"unexpected }")}', 'interpol'),
        ('bad-delimiter', 'fn main(){print(f"{(42]}")}', ''),
        ('array', 'fn main(){values:=[1,2];print(f"{values}")}', 'str'),
        ('record', 'struct P{x:i32} fn main(){print(f"{P{x:1}}")}', 'str'),
        ('enum', 'enum P{A,B} fn main(){print(f"{P.A}")}', 'str'),
        ('unit', 'fn nothing(){} fn main(){print(f"{nothing()}")}', 'str'),
    )
    checked = 0
    for name, text, message in invalid:
        source = work / f'invalid-{name}.ddr'
        source.write_text(text + '\n')
        for target in TARGETS:
            result = call(compiler, ['check', source, '--agent', '--target', target], work, 1)
            report = json.loads(result.stdout)
            assert report['kind'] == 'error' and report['path'] == str(source), report
            assert 0 <= report['start'] <= report['end'] <= len(source.read_bytes()), report
            if message:
                assert message in report['message'].lower(), report
            image = work / 'previous-image'
            image.write_bytes(b'previous image')
            built = call(compiler, ['build', source, '--target', target, '-o', image], work, 1)
            assert report['message'] in built.stderr, (report, built.stderr)
            assert image.read_bytes() == b'previous image'
            checked += 1

    # Unknown names inside a hole must map to the imported original file and
    # byte offset, even after UTF-8 text and module namespace expansion.
    source = work / 'broken.ddr'
    text = 'pub fn broken() -> string {\n    value := 42\n    return f"å 🌍 {value} {missing_value}"\n}\n'
    source.write_text(text)
    main = work / 'diagnostic-main.ddr'
    main.write_text('import broken\nfn main(){print(broken.broken())}\n')
    start = text.encode().index(b'missing_value')
    line = text[:text.index('missing_value')].count('\n') + 1
    prefix = text[:text.index('missing_value')].rsplit('\n', 1)[-1]
    column = len(prefix) + 1
    for target in TARGETS:
        for workers in ('1', '4') if target == 'arm64-macos' and host_target() == target else ('1',):
            flags = ['--target', target, '--jobs', workers]
            result = call(compiler, ['check', main, '--agent', *flags], work, 1)
            report = json.loads(result.stdout)
            assert report['path'] == str(source) and report['start'] == start, report
            assert report['end'] == start + len('missing_value'), report
            plain = call(compiler, ['check', main, *flags], work, 1)
            assert f'{source}:{line}:{column}:' in plain.stderr, plain.stderr
            checked += 1
    return checked


def api_contracts(compiler, work, host):
    api = work / 'api-consumer'
    call(compiler, ['build', FIXTURES / 'api.ddr', '-o', api], ROOT)
    direct = work / 'direct.ddr'
    direct.write_text('fn main(){value:=40;print(f"å \\n{{{value + 2}}} {f\'nested {true}\'} \\0") }\n')
    direct_output = 'å \n{42} nested true \0\n'
    checked = 0
    for number, target in enumerate(TARGETS):
        for mode, source, expected in (
            ('direct', direct, direct_output),
            ('source', FIXTURES / 'main.ddr', 'interpolation ok\n'),
            ('tokens', FIXTURES / 'main.ddr', 'interpolation ok\n'),
        ):
            image = work / f'api-{target}-{mode}'
            call(api, [source, number, mode, image], work, expected='api ok\n')
            assert_image(image, target)
            if target == host:
                call(image, [], work, expected=expected)
            checked += 1
    return checked


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    builds = 0
    with tempfile.TemporaryDirectory(prefix='dunder-interpolation-') as temporary:
        work = Path(temporary).resolve()
        source = FIXTURES / 'main.ddr'
        # Each mode independently exercises the same behavior; the mixed modes
        # prevent a feature that works only in one optimized frontend path.
        modes = (
            [],
            ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'],
            ['--scalar-opt=on', '--ownership-opt=off', '--mir=on'],
            ['--scalar-opt=off', '--ownership-opt=on', '--mir=on'],
        )
        for target in TARGETS:
            for index, mode in enumerate(modes):
                jobs = ('1', '4') if target == host == 'arm64-macos' else ('1',)
                for workers in jobs:
                    image = work / f'fixture-{target}-{index}-{workers}'
                    flags = ['--target', target, '--jobs', workers, *mode]
                    call(compiler, ['build', source, '-o', image, *flags], ROOT)
                    assert_image(image, target)
                    if target == host:
                        call(image, [], work, expected='interpolation ok\n')
                    builds += 1
            # Cached and fresh compilation must publish exactly the same image.
            cached = work / f'cached-{target}'
            flags = ['--target', target, '--incremental', '--timings=json']
            call(compiler, ['build', source, '-o', cached, *flags], ROOT)
            first = cached.read_bytes()
            call(compiler, ['build', source, '-o', cached, *flags], ROOT)
            assert cached.read_bytes() == first, target
            assert_image(cached, target)
            if target == host:
                call(cached, [], work, expected='interpolation ok\n')
            builds += 2
        invalid = diagnostic_contracts(compiler, work)
        apis = api_contracts(compiler, work, host)
    print(f'interpolation: {builds} CLI builds, {invalid} diagnostic contracts, '
          f'{apis} source/token API images; host execution passed')


if __name__ == '__main__':
    main()
