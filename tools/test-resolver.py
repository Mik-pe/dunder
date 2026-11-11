#!/usr/bin/env python3
"""Exercise structural iterator scope through real module consumers."""
import argparse
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/resolver_scopes'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
EXPECTED = '0\n1\n2\n3\n3\n3\n3\n3\n3\n5\n6\n3\n99\n'
INVALID = (
    ('struct Bound{value:i32}\nfn main(){for index in (Bound{value:0}.value)..1{};print(index)}', "unknown local 'index'"),
    ('struct Bound{value:i32}\nfn main(){for index in Bound{value:0}.value..1{index=1}}', 'assignment requires a mutable stored value'),
    ('fn scope(n:i32)->i32{for index in 0..n{};return index}\nfn main(){}', "unknown local 'index'"),
)


def call(compiler, args, work, status=0):
    result = subprocess.run([str(compiler), *map(str, args)], cwd=work, env={'PATH': ''},
                            capture_output=True, text=True, timeout=30)
    if result.returncode != status:
        raise AssertionError(f'{args} returned {result.returncode}, expected {status}\n'
                             f'{result.stdout}{result.stderr}')
    return result


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return 'arm64-macos'
    if system == 'Linux' and machine in ('aarch64', 'arm64'):
        return 'arm64-linux'
    if system == 'Linux' and machine == 'x86_64':
        return 'x86_64-linux'
    raise RuntimeError(f'unsupported execution host: {system} {machine}')


def wasm(path, expected, work):
    result = subprocess.run(['node', str(ROOT / 'tools/runtime-contract-wasm.cjs'), str(path)],
                            cwd=work, capture_output=True, text=True, timeout=30)
    if result.returncode or result.stdout != expected:
        raise AssertionError(f'Wasm execution: {result.returncode}\n{result.stdout}{result.stderr}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='also compare unaffected scope-error diagnostics')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    host = host_target()
    with tempfile.TemporaryDirectory(prefix='dunder-resolver-') as temporary:
        work = Path(temporary)
        for fixture in FIXTURES.glob('*.ddr'):
            shutil.copyfile(fixture, work / fixture.name)
        source = work / 'main.ddr'
        for target in TARGETS:
            modes = []
            for mode in ('off', 'on'):
                flags = ['--target', target, f'--scalar-opt={mode}']
                checked = call(compiler, ['check', source, '--agent', *flags], work)
                records = [json.loads(line) for line in checked.stdout.splitlines()]
                if not records or records[-1]['kind'] != 'ok':
                    raise AssertionError(f'missing successful check: {checked.stdout}')
                modes.append(checked.stdout)
                image = work / f'consumer-{target}-{mode}'
                call(compiler, ['build', source, '-o', image, *flags], work)
                if target == host:
                    actual = subprocess.run([str(image)], cwd=work, env={'PATH': ''},
                                            capture_output=True, text=True, timeout=10)
                    if actual.returncode or actual.stdout != EXPECTED:
                        raise AssertionError(f'native execution: {actual.stdout}{actual.stderr}')
                elif target == 'wasm32':
                    wasm(image, EXPECTED, work)
            if modes[0] != modes[1]:
                raise AssertionError(f'optimization changed binding records for {target}')
        # Test builds flatten source text instead of forwarding the ordinary
        # module token stream. Exercise the same scope contract through it.
        tests = work / 'cases.ddr'
        tests.write_text(source.read_text() + '\n@test fn iterator_scope(){'
                         'for i in (Bound{value:0}.value)..i(){assert(i>=0&&i<3)}}\n')
        for mode in ('off', 'on'):
            result = call(compiler, ['test', tests, '--target', host, f'--scalar-opt={mode}',
                                    '-o', work / f'suite-{mode}'], work)
            if '1 passed, 0 failed' not in result.stdout:
                raise AssertionError(result.stdout)
            image = work / f'test-{mode}.wasm'
            call(compiler, ['build-tests', tests, '--group', '0', '--target', 'wasm32',
                            f'--scalar-opt={mode}', '-o', image], work)
            wasm(image, '', work)
        for number, (text, message) in enumerate(INVALID):
            invalid = work / f'invalid-{number}.ddr'
            invalid.write_text(text)
            for target in TARGETS:
                outputs = []
                for mode in ('off', 'on'):
                    options = ['check', invalid, '--agent', '--target', target, f'--scalar-opt={mode}']
                    result = call(compiler, options, work, status=1)
                    records = [json.loads(line) for line in result.stdout.splitlines()]
                    if len(records) != 1 or records[0]['kind'] != 'error' or message not in records[0]['message']:
                        raise AssertionError(result.stdout)
                    outputs.append(result.stdout)
                    if baseline:
                        previous = call(baseline, options, work, status=1)
                        if (previous.stdout, previous.stderr) != (result.stdout, result.stderr):
                            raise AssertionError(f'scope diagnostic changed: {previous.stdout}{result.stdout}')
                if outputs[0] != outputs[1]:
                    raise AssertionError(f'optimization changed scope diagnostic for {target}')
    print('Resolver scopes passed: all four targets, native/Wasm, both modes and test bodies')


if __name__ == '__main__':
    main()
