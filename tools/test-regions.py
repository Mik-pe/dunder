#!/usr/bin/env python3
"""Exercise inactive scalar region placement, control flow, traps and ABI."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def call(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=60)
    if result.returncode != status:
        raise AssertionError(f'{command}: {result.returncode}, expected {status}\n'
                             f'{result.stdout}{result.stderr}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='Compare normal compilation byte-for-byte with this pre-refactor compiler.')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-regions-') as temporary:
        work = Path(temporary); harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/scalar-region-emitter.ddr', '-o', harness], work)
        results = []
        for selected in range(20):
            status = {8: 101, 9: 102, 10: 101, 11: 102, 12: 101}.get(selected, 0)
            for target in range(4):
                for ranged in ('off', 'on'):
                    for const_div in ('off', 'on'):
                        image = work / f'{selected}-{target}-{ranged}-{const_div}'
                        result = call([harness, image, target, selected, ranged, const_div], work)
                        actual, required, _ = map(int, result.stdout.split())
                        if actual < 0 or required < 621 or required > 65535:
                            raise AssertionError('region placement/size contract failed')
                        results.append({'case': selected, 'target': target, 'range': ranged, 'const_div': const_div, 'words': actual, 'required_slots': required, 'sha256': hashlib.sha256(image.read_bytes()).hexdigest()})
                        if target == host:
                            call([image], work, status)
                        elif target == 3:
                            call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work,
                                 125 if status else 0)
        for refused in range(20, 26):
            for target in range(4):
                image = work / f'refused-{refused}-{target}'
                call([harness, image, target, refused, 'on', 'on'], work)
                if image.exists():
                    raise AssertionError('refused region changed/published Code')
        identities = []
        if args.baseline:
            baseline = args.baseline.resolve(strict=True)
            sources = [
                'fn calc(x:i64,y:i64)->i64{(x*x+y)/7%11} fn main(){assert(calc(4,3)==2);print(42)}',
                'fn calc(n:i32)->i32{var sum:=0;for i in 0..n{if i==3{continue};sum=sum+i};sum} fn main(){print(calc(8))}',
                'fn calc(x:i64)->i64{var i:=0;var sum:i64=1;while i<10{sum=(sum*3+x)/7;i=i+1};sum} fn main(){assert(calc(9)>0);print(42)}',
                'fn calc(values:&[]i64)->i64{var i:=0;var sum:i64=0;while i<values.len{sum=sum+values[i];i=i+1};sum} fn main(){values:[]i64=[1,2,3];assert(calc(&values)==6);print(42)}',
                'fn update(values:&mut []i64){for i in 0..values.len{values[i]=values[i]+1}} fn main(){var values:[]i64=[1,2,3];update(&mut values);assert(values[2]==4);print(42)}',
                'fn calc(n:i32)->i32{var sum:=0;for i in 0..n{var j:=0;while j<i{j=j+1;if j==2{continue};sum=sum+j;if sum>100{return sum}}};sum} fn main(){print(calc(5))}',
            ]
            pressure = 'fn calc(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32)->i32{'
            for local in range(18):
                pressure += f'var v{local}:=' + 'abcdefgh'[local % 8] + f'+{local};'
            pressure += '+'.join(f'v{local}' for local in range(18)) + '} fn main(){print(calc(1,2,3,4,5,6,7,8))}'
            sources.append(pressure)
            for case, text in enumerate(sources):
                source = work / f'normal-{case}.ddr'; source.write_text(text)
                for target, target_name in enumerate(('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')):
                    for ranged, const_div, frame_free, inlined in itertools.product(('off', 'on'), repeat=4):
                        switches = ['--target', target_name, '--inline-opt=' + inlined, '--range-opt=' + ranged, '--const-div-opt=' + const_div, '--frame-free=' + frame_free]
                        before = work / 'normal-before'; after = work / 'normal-after'
                        call([baseline, 'build', source, *switches, '-o', before], work)
                        call([compiler, 'build', source, *switches, '-o', after], work)
                        if before.read_bytes() != after.read_bytes():
                            raise AssertionError(f'normal output changed:case{case},target{target},range{ranged},const{const_div},frame{frame_free},inline{inlined}')
                        identities.append({'case': case, 'target': target, 'range': ranged, 'const_div': const_div, 'frame_free': frame_free, 'inline': inlined, 'sha256': hashlib.sha256(after.read_bytes()).hexdigest()})
        if args.report:
            args.report.write_text(json.dumps({'regions': results, 'normal_identities': identities}, indent=2) + '\n')
    print('regions:320 all-target images,160 native/Wasm runs,24 untouched placement refusals; imports/exports, shadowing, break/continue, zero iterations, pressure/high frames, traps/order and ABI passed')
    if args.baseline:
        print('normal compiler emission:448 source/target/policy images byte-identical')


if __name__ == '__main__':
    main()
