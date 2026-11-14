#!/usr/bin/env python3
"""Whole-record take oracle, ownership/capacity contracts, no latency claims.

The emitter fixture directly exercises the NEW helper with actual legacy
runtime clones as its comparison. Source fixtures also run through the supplied
compiler; until frontend integration those are baseline contracts, not evidence
that the new whole-expression hook has been activated.
"""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
import wasm_host

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
REPEAT_WASM = r'''
const fs=require('node:fs'),assert=require('node:assert/strict');
const p=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(process.argv[1])),
 {dunder:{print_i32(v){assert.equal(v,42)}}}).exports;
assert.equal(Number(p.main()||0),0); const bytes=p.memory.buffer.byteLength;
for(let n=0;n<20;n++)assert.equal(Number(p.main()||0),0);
assert.equal(p.memory.buffer.byteLength,bytes);
'''


def call(command, status=0, output=None):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, status, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    js_bin = wasm_host.js_runtime()
    with tempfile.TemporaryDirectory(prefix='dunder-record-take-') as directory, wasm_host.WasmWorker(js_bin) as wasm_worker:
        work = Path(directory)
        call([compiler, 'test', ROOT / 'tests/record_take.ddr', '-o', work / 'units'])
        call([compiler, 'build-tests', ROOT / 'tests/record_take.ddr', '--target', 'wasm32', '-o', work / 'units.wasm'])
        call([js_bin, ROOT / 'tools/test-wasm.mjs', '--execute', work / 'units.wasm'])
        emitter = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/record-take-emitter.ddr', '-o', emitter])
        for n in (0, 1, 8, 9, 17, 64):
            profiles = {}
            for target in range(4):
                for mode in ('clone', 'take'):
                    image = work / f'{target}-{mode}-{n}'
                    # Exactly33 words for two requested helpers; the unrelated
                    # array-bearing record has a reserved label but NO body.
                    call([emitter, image, target, mode, n], output=b'33\n')
                    if target == host:
                        run = call([image], output=b'')
                        profile = json.loads(run.stderr)
                        assert profile['kind'] == 'runtime_profile'
                        profiles[mode] = profile['counters']
                    elif target == 3:
                        run = call([js_bin, '-e', REPEAT_WASM, image], output=b'')
                        assert not run.stderr
            old, new = profiles['clone'], profiles['take']
            assert old['allocation_count'] - new['allocation_count'] == (10 if n == 0 else 11)
            assert old['array_clone_calls'] - new['array_clone_calls'] == 4
            assert old['array_clone_bytes'] - new['array_clone_bytes'] == 8 * n + 38
            assert old['record_clone_calls'] - new['record_clone_calls'] == 3
            assert old['record_clone_bytes'] - new['record_clone_bytes'] == 80
            assert old['retain_calls'] - new['retain_calls'] == 4
            print(f'PASS len{n}: four target images, native/Wasm ownership/capacity oracle; '
                  f'{old["allocation_count"] - new["allocation_count"]} allocations and '
                  f'{8*n+38} array-copy bytes removed', flush=True)
        for target, name in enumerate(TARGETS):
            image = work / f'source-{target}'
            call([compiler, 'build', ROOT / 'tests/fixtures/record-take-values.ddr', '--target', name, '-o', image])
            if target == host:
                call([image], output=b'42\n')
            elif target == 3:
                res = wasm_worker.run(image, repeat=True, binary_output=True)
                assert res.returncode == 0 and res.stdout == b'42\n', res
        # Scalar traps in child expressions must happen in source order. The
        # native exit code distinguishes the first trap; Wasm's unreachable
        # maps to125, with the same observable print prefix and no later call.
        prefix = '''struct Item { values: []i64, title: string }
fn failure() -> Item { print(11); assert(false); Item { values: [1], title: "bad" } }
fn later() -> i32 { print(22); z := 0; 1 / z }
fn accept(item: Item, ignored: i32) -> i32 { i32(item.values[0]) + ignored }
'''
        traps = {
            'arg-order': (104, 'fn main() { unused := accept(failure(), later()) }'),
            'field-order': (104, 'fn main() { unused := Item { values: failure().values, title: later().str() } }'),
            'projection-bounds': (103, 'fn main() { print(11); unused := Item { values: [1], title: "x" }.values[2]; ignored := later() }'),
        }
        for case, (status, body) in traps.items():
            source = work / f'{case}.ddr'
            source.write_text(prefix + body + '\n')
            for target, name in enumerate(TARGETS):
                image = work / f'{case}-{target}'
                call([compiler, 'build', source, '--target', name, '-o', image])
                if target == host:
                    call([image], status=status, output=b'11\n')
                elif target == 3:
                    res = wasm_worker.run(image, binary_output=True)
                    assert res.returncode == 125 and res.stdout == b'11\n', res
        print('PASS source snapshot/projection/callback/branch/return/reentry contracts '
              '(new frontend hook is tested only when compiler integrates record_take)', flush=True)
        print('PASS source child-trap ordering (native exact status; Wasm exact print prefix)', flush=True)


if __name__ == '__main__':
    main()
