#!/usr/bin/env python3
"""Execute complete MIR promotion through all four target image lowerers."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[1]
def call(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True, text=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64','aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-mir-promotion-') as directory:
        work = Path(directory); harness = work / 'emitter'
        call([compiler,'build',ROOT/'tests/fixtures/mir-promotion-emitter.ddr','-o',harness],work)
        for selected in range(12):
            status = {8:101,10:102}.get(selected,0)
            for target in range(4):
                for mode in ('off','on'):
                    image = work / f'{selected}-{target}-{mode}'
                    result = call([harness,image,target,selected,mode],work)
                    homes, memory = map(int,result.stdout.split())
                    assert 0 <= homes <= 4
                    if mode == 'on':
                        assert homes > 0
                        if selected != 1: assert memory == 0, (selected,target,memory)
                    else: assert homes == 0 and memory > 0
                    if target == host: call([image],work,status)
                    if target == 3: call(['node',ROOT/'tools/runtime-contract-wasm.cjs',image],work,125 if status else 0)
            print('PASS',selected,'all targets; native/Wasm executed; preserved homes and original result',flush=True)
    print('96 whole-function MIR images verified')
if __name__ == '__main__': main()
