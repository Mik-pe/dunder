#!/usr/bin/env python3
"""Preserve prior raw frame bytes observed by a call before original stores."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[1]
def call(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True, text=True, timeout=60)
    assert result.returncode == status, (command,result.returncode,status,result.stdout,result.stderr)
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler',type=Path)
    parser.add_argument('--expect-bug',action='store_true',help='red check against the old zero-initialized homes')
    args=parser.parse_args(); compiler=args.compiler.resolve(strict=True)
    host=0 if platform.system()=='Darwin' else (1 if platform.machine() in ('arm64','aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-mir-entry-') as directory:
        work=Path(directory); harness=work/'emitter'
        call([compiler,'build',ROOT/'tests/fixtures/mir-entry-emitter.ddr','-o',harness],work)
        for size in ('small','large'):
            for target in range(4):
                for mode in ('off','on'):
                    image=work/f'{size}-{target}-{mode}'
                    result=call([harness,image,target,size,mode],work)
                    assert int(result.stdout)==(3 if mode=='on' else 0)
                    status=104 if args.expect_bug and mode=='on' else 0
                    if target==host: call([image],work,status)
                    if target==3: call(['node',ROOT/'tools/runtime-contract-wasm.cjs',image],work,125 if status else 0)
            print('PASS',size,'known prior frame bits; four targets; native/Wasm observed-before-store',flush=True)
    print('16 entry-preservation images verified'+(' (old bug reproduced)' if args.expect_bug else ''))
if __name__=='__main__': main()
