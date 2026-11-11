#!/usr/bin/env python3
"""Check string-operand lifetime proofs through real compiler consumers."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
INVALID = (
    'fn main(){print("a"==true)}',
    'fn unused()->string{true} fn main(){print("a"=="b")}',
    'fn main(){value:="a";value="b"}',
    'fn main(){values:=["a"];values[0]="b"}',
    'fn main(){print("a"==missing)}',
)
TRAPS = (
    ('fn main(){a:=["a"];print("a"==a[2])}', 103),
    ('fn main(){a:=["a"];zero:=0;print("a"==a[1/zero])}', 102),
)


def call(compiler, arguments, work, status=0):
    result = subprocess.run([str(compiler), *map(str, arguments)], cwd=work,
                            capture_output=True, text=True, timeout=60)
    if result.returncode != status:
        raise AssertionError(f'{arguments}: {result.returncode}, expected {status}\n'
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
    raise RuntimeError(f'unsupported execution host {system} {machine}')


def execute(image, target, work, status=0, expected='42\n', repeat=False):
    if target == 'wasm32':
        command = ['node', str(ROOT / 'tools/runtime-contract-wasm.cjs'), str(image)]
        if repeat:
            command.append('--repeat')
        if status:
            status = 125  # The portable runner reports Wasm traps uniformly.
    else:
        command = [str(image)]
    result = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=30)
    if result.returncode != status or result.stdout != expected:
        raise AssertionError(f'{target}: {result.returncode}, {result.stdout!r}\n{result.stderr}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    host = host_target()
    with tempfile.TemporaryDirectory(prefix='dunder-ownership-') as temporary:
        work = Path(temporary)
        source = work / 'main.ddr'
        source.write_text((ROOT / 'tests/fixtures/ownership_leases.ddr').read_text())
        for target in TARGETS:
            records = []
            for scalar in ('off', 'on'):
                for ownership in ('off', 'on'):
                    flags = ['--target', target, f'--scalar-opt={scalar}',
                             f'--ownership-opt={ownership}']
                    checked = call(compiler, ['check', source, '--agent', *flags], work)
                    parsed = [json.loads(line) for line in checked.stdout.splitlines()]
                    if not parsed or parsed[-1]['kind'] != 'ok':
                        raise AssertionError(checked.stdout)
                    records.append(checked.stdout)
                    image = work / f'fixture-{target}-{scalar}-{ownership}'
                    call(compiler, ['build', source, '-o', image, *flags], work)
                    if target in (host, 'wasm32'):
                        execute(image, target, work, repeat=target == 'wasm32')
            if len(set(records)) != 1:
                raise AssertionError(f'{target}: ownership or scalar optimization changed bindings')
        for index, text in enumerate(INVALID):
            invalid = work / f'invalid-{index}.ddr'
            invalid.write_text(text)
            for target in TARGETS:
                outputs = []
                build_errors = []
                for ownership in ('off', 'on'):
                    flags = ['--target', target, f'--ownership-opt={ownership}']
                    result = call(compiler, ['check', invalid, '--agent', *flags], work, 1)
                    outputs.append(result.stdout)
                    message = json.loads(result.stdout)['message']
                    image = work / 'previous-image'
                    image.write_bytes(b'previous image')
                    built = call(compiler, ['build', invalid, '-o', image, *flags], work, 1)
                    if message not in built.stderr or image.read_bytes() != b'previous image':
                        raise AssertionError(f'{invalid.name}: build diagnostic/output preservation')
                    build_errors.append(built.stderr)
                if len(set(outputs)) != 1:
                    raise AssertionError(f'{invalid.name}: changed diagnostic for {target}: {outputs}')
                if len(set(build_errors)) != 1:
                    raise AssertionError(f'{invalid.name}: changed build diagnostic for {target}')
                if baseline:
                    original = call(baseline, ['check', invalid, '--agent', '--target', target], work, 1)
                    if original.stdout != outputs[0]:
                        raise AssertionError(f'{invalid.name}: baseline diagnostic differs')
        for number, (text, status) in enumerate(TRAPS):
            source.write_text(text)
            for target in (host, 'wasm32'):
                for ownership in ('off', 'on'):
                    image = work / f'trap-{number}-{target}-{ownership}'
                    call(compiler, ['build', source, '-o', image, '--target', target,
                                    f'--ownership-opt={ownership}'], work)
                    execute(image, target, work, status=status, expected='')
    print('ownership leases: all targets, binding/diagnostic equality, lifetimes and traps passed')


if __name__ == '__main__':
    main()
