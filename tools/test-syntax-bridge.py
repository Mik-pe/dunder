#!/usr/bin/env python3
"""Verify mut/move syntax, parameter offsets and public source/token compiler APIs."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/syntax_bridge'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


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
    raise RuntimeError(f'unsupported host {system}/{machine}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    builds, errors = 0, 0
    with tempfile.TemporaryDirectory(prefix='dunder-syntax-bridge-') as temporary:
        work = Path(temporary).resolve()
        source = FIXTURES / 'main.ddr'
        modes = ([], ['--scalar-opt=off', '--inline-opt=off', '--mir=off'])
        for target in TARGETS:
            for mode in modes:
                images = []
                for workers in ('1', '4') if target == host == TARGETS[0] else ('1',):
                    image = work / f'image-{builds}'
                    call(compiler, ['build', source, '--target', target, '--jobs', workers,
                                    *mode, '-o', image], ROOT)
                    if target == host:
                        call(image, [], work, expected='syntax bridge ok\n')
                    images.append(image.read_bytes())
                    builds += 1
                assert all(data == images[0] for data in images), target
            records = call(compiler, ['check', source, '--agent', '--target', target], ROOT)
            reports = [json.loads(line) for line in records.stdout.splitlines()]
            assert reports[-1]['kind'] == 'ok', reports
            marked = (FIXTURES / 'helper.ddr').read_bytes().index(b'move number: i32') + len(b'move ')
            positions = [row for row in reports if row.get('kind') == 'binding'
                         and row.get('path') == str(FIXTURES / 'helper.ddr')
                         and row.get('name') == 'number']
            assert len(positions) == 1 and positions[0]['start'] == marked, positions
            assert all(row.get('name') != 'move' for row in reports), reports

        # Reuse the public compiler-API consumer to exercise both loader
        # representations and direct source tokens, independently of the CLI.
        api = work / 'api'
        call(compiler, ['build', ROOT / 'tests/fixtures/interpolation/api.ddr', '-o', api], ROOT)
        direct = work / 'direct.ddr'
        direct.write_text('fn add(move x:i32)->i32{move x+1} '
                          'fn main(){mut x:=40;x+=1;f:fn(move i32)->i32=add;print(f(move x))}\n')
        for number, target in enumerate(TARGETS):
            for mode, path, expected in (
                ('direct', direct, '42\n'),
                ('source', source, 'syntax bridge ok\n'),
                ('tokens', source, 'syntax bridge ok\n'),
            ):
                image = work / f'api-{number}-{mode}'
                call(api, [path, number, mode, image], work, expected='api ok\n')
                if target == host:
                    call(image, [], work, expected=expected)
                builds += 1

        invalid = (
            'fn main(){mut 42}',
            'fn main(){value:=move }',
            'fn bad(move :i32){} fn main(){}',
            'fn main(){f:fn(move)->i32=missing}',
        )
        for index, text in enumerate(invalid):
            path = work / f'invalid-{index}.ddr'
            path.write_text(text + '\n')
            for target in TARGETS:
                result = call(compiler, ['check', path, '--agent', '--target', target], work, 1)
                report = json.loads(result.stdout)
                assert report['kind'] == 'error' and report['path'] == str(path), report
                assert 0 <= report['start'] <= report['end'] <= len(path.read_bytes()), report
                errors += 1
    print(f'syntax bridge: {builds} native images, {errors} rejected syntax contracts; '
          'host execution, import shadows, parameter offsets and source/token APIs passed')


if __name__ == '__main__':
    main()
