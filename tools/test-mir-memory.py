#!/usr/bin/env python3
"""Verify explicit private-storage context, raw fallback and call synchronization."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile
ROOT = Path(__file__).resolve().parents[1]
def call(command, work):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64','aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-mir-memory-') as directory:
        work = Path(directory); harness = work / 'emitter'
        call([compiler,'build',ROOT/'tests/fixtures/mir-memory-emitter.ddr','-o',harness],work)
        for selected in range(8):
            for target in range(4):
                images, counters = {}, {}
                for policy in ('legacy','raw','source'):
                    image = work / f'{selected}-{target}-{policy}'
                    lines = call([harness,image,target,selected,policy],work).stdout.split()
                    ready = lines[0] == 'true'
                    homes, skipped, frame_ops, object_ops, synchronized = map(int,lines[1:])
                    assert object_ops == 2
                    if policy == 'source' and selected not in (3,4):
                        assert ready and homes > 0 and skipped == 2
                        if selected in (0,2,6,7): assert frame_ops == 0
                        if selected == 5: assert homes == 2 and frame_ops > 0
                        if selected in (1,2): assert synchronized == 1
                    else:
                        assert not ready and homes == skipped == synchronized == 0 and frame_ops > 0
                    counters[policy] = lines
                    images[policy] = image.read_bytes()
                    if target == host: call([image],work)
                    if target == 3: call(['node',ROOT/'tools/runtime-contract-wasm.cjs',image],work)
                assert images['legacy'] == images['raw'] and counters['legacy'] == counters['raw']
                assert (images['source'] != images['raw']) == (selected not in (3,4))
            print('PASS',selected,'all targets; native/Wasm; raw identity, object reads/writes, calls and homes',flush=True)
    print('96 private-memory images verified; no timing series')
if __name__ == '__main__': main()
