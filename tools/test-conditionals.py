#!/usr/bin/env python3
"""Check structural if-condition boundaries, shadowing and source diagnostics."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
HOST = 'arm64-macos' if platform.system() == 'Darwin' else ('arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('compiler', type=Path)
args = parser.parse_args()
compiler = args.compiler.resolve(strict=True)

def invoke(arguments):
    return subprocess.run([compiler, *map(str, arguments)], capture_output=True, text=True, timeout=30)

with tempfile.TemporaryDirectory(prefix='dunder-conditionals-') as directory:
    work = Path(directory)
    deep = work / 'deep.ddr'
    deep.write_text('struct Cond{value:bool}\nfn value()->i32{if ' + '(' * 60 + 'Cond{value:[true][0]}.value' + ')' * 60 + '{42}else{0}}\nfn main(){print(value())}\n')
    for mode in ('on', 'off'):
        for target in TARGETS:
            options = [f'--scalar-opt={mode}', '--target', target]
            for source in (ROOT / 'tests/contracts/conditional_boundaries.ddr', deep):
                checked = invoke(['check', source, *options, '--agent'])
                assert checked.returncode == 0, (mode, target, source, checked.stderr, checked.stdout)
                output = work / f'{source.stem}-{mode}-{target}'
                built = invoke(['build', source, *options, '-o', output])
                assert built.returncode == 0, (mode, target, source, built.stderr, built.stdout)
                if target == HOST or target == 'wasm32':
                    command = [output] if target == HOST else ['node', ROOT / 'tools/runtime-contract-wasm.cjs', output]
                    run = subprocess.run(command, capture_output=True, text=True, timeout=10)
                    assert run.returncode == 0 and run.stdout == '42\n', (mode, target, source, run.returncode, run.stdout, run.stderr)
            for source in sorted((ROOT / 'tests/fixtures/conditional_errors').glob('*.ddr')):
                expected = source.read_text().splitlines()[0].removeprefix('// error: ')
                output = work / 'preserved-image'
                output.write_bytes(b'previous successful output')
                for command in ('check', 'build'):
                    extras = ['--agent'] if command == 'check' else ['-o', output]
                    failed = invoke([command, source, *options, *extras])
                    message = failed.stderr
                    if command == 'check':
                        diagnostic = json.loads(failed.stdout)
                        assert diagnostic['kind'] == 'error' and Path(diagnostic['path']).resolve() == source
                        assert 0 <= diagnostic['start'] <= diagnostic['end'] <= source.stat().st_size
                        message = diagnostic['message']
                    assert failed.returncode == 1 and expected in message, (mode, target, source, command, failed.returncode, failed.stdout, failed.stderr)
                    assert output.read_bytes() == b'previous successful output'
            print('PASS', target, f'scalar-opt={mode}', 'constructors/groups/shadowing/diagnostics', flush=True)
print('All four targets and both scalar modes checked/built; host+Wasm executed; diagnostic outputs preserved')
