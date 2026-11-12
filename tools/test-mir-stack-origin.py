#!/usr/bin/env python3
"""Untimed four-target origin roundtrip and real Wasm stack-fault witnesses."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=True)
    assert not any(work.iterdir()), 'preserve earlier outcomes; choose an empty directory'
    receipt = {'protocol': 'mir-stack-origin-v1', 'timings': 'none',
               'compiler': str(compiler), 'compiler_sha256': digest(compiler),
               'commands': [], 'images': [], 'complete': False}

    def save():
        (work / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')

    def call(command):
        result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                                text=True, timeout=30)
        receipt['commands'].append({'argv': list(map(str, command)), 'exit': result.returncode,
                                    'stdout': result.stdout, 'stderr': result.stderr})
        save()
        assert result.returncode == 0, receipt['commands'][-1]
        return result

    emitter = work / 'emitter'
    source = ROOT / 'tests/fixtures/mir-stack-origin-emitter.ddr'
    receipt['emitter_source_sha256'] = digest(source)
    call([compiler, 'build', source, '-o', emitter])
    js = work / 'execute.mjs'
    js.write_text('''import fs from 'node:fs';
const printed=[];
const instance=(await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),
  {dunder:{print_i64:x=>printed.push(x.toString())}})).instance;
let result=null,error=null;
try{result=instance.exports.main()}catch(e){
  if(!(e instanceof WebAssembly.RuntimeError))throw e;
  error={name:e.name,message:e.message};
}
console.log(JSON.stringify({result,error,printed}));
''')
    for target in range(4):
        images = []
        for origin in ('raw', 'temp'):
            image = work / f'valid-{target}-{origin}'
            call([emitter, image, target, origin, 'valid'])
            images.append(image)
            row = {'target': target, 'origin': origin, 'path': str(image), 'sha256': digest(image)}
            if target == 0 and platform.system() == 'Darwin' and platform.machine() in ('arm64', 'aarch64'):
                result = call([image]); assert result.stdout == '42\n' and result.stderr == ''
                row['execution'] = 'host passed:42'
            elif target == 3:
                result = json.loads(call(['node', js, image]).stdout)
                assert result == {'result': 0, 'error': None, 'printed': ['42']}, result
                row['execution'] = 'Wasm passed:42'
            else:
                row['execution'] = 'cross-built only; no execution claim'
            receipt['images'].append(row); save()
        assert images[0].read_bytes() == images[1].read_bytes(), target
    for mode in ('push-fault', 'pop-fault', 'push-elided-control', 'pop-elided-control'):
        images = []
        for origin in ('raw', 'temp'):
            image = work / f'{mode}-{origin}.wasm'
            call([emitter, image, 3, origin, mode]); images.append(image)
            result = json.loads(call(['node', js, image]).stdout)
            assert result['printed'] == [], result
            if mode.endswith('-fault'):
                assert result['result'] is None and result['error']['name'] == 'RuntimeError', result
            else:
                assert result == {'result': 77, 'error': None, 'printed': []}, result
            receipt['images'].append({'target': 3, 'origin': origin, 'mode': mode,
                                      'sha256': digest(image), 'execution': result}); save()
        assert images[0].read_bytes() == images[1].read_bytes(), mode
    assert digest(compiler) == receipt['compiler_sha256']
    receipt['complete'] = True; save()
    print('PASS:16 exact raw/temp target images; host+Wasm success, real raw/temp Wasm push/pop faults and nonfaulting elision controls; no timings')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == '__main__':
    main()
