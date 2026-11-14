#!/usr/bin/env python3
"""Exercise register-aware arrays, exact saved addresses and scratch contracts."""
import argparse
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import wasm_host
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('compiler', type=Path)
args = parser.parse_args()
compiler = args.compiler.resolve(strict=True)
host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
normal = [5, 0, 11, 13, 17, 19, 23, 29]

def balanced(leaves):
    if leaves == 1:
        return 'p2'
    half = leaves // 2
    return f'({balanced(half)}+{balanced(leaves-half)})'

padding = ''.join(f'pad{i}:i64={i};' for i in range(40))
# name, body, layout, args, result, status, length, PUSHx8, ADDaddr9, directSTRhome
stores = [
    ('home_value', '{p0[p1]=p2;0}', 'homes', normal, 0, 0, 8, 0, 0, 1),
    ('read_value', '{p0[p1]=p0[p1];0}', 'homes', normal, 0, 0, 8, 0, 1, 0),
    ('nested_reads', '{p0[p1]=p0[p1+1]+p0[p1+2];p0[p1]}', 'homes', normal, 4, 0, 8, 0, 1, 0),
    ('read_constdiv', '{p0[p1]=p0[p1]/3;p0[p1]}', 'homes', normal, 1, 0, 8, 0, 1, 0),
    ('dynamic_div', '{p0[p1]=p2/p3;p0[p1]}', 'homes', normal, 0, 0, 8, 0, 0, 0),
    ('multiply', '{p0[p1]=p0[p1]+p2*p3;p0[p1]}', 'homes', normal, 146, 0, 8, 0, 1, 0),
    ('small_spills', '{p0[p1]=p0[p1]+p2;p0[p1]}', 'spill', normal, 14, 0, 8, 0, 1, 0),
    ('large_spill_read', '{'+padding+'p0[p1]=p0[p1]+pad39;p0[p1]}', 'spill', normal, 42, 0, 8, 1, 0, 0),
    ('large_spill_scalar', '{'+padding+'p0[p1]=pad39;p0[p1]}', 'spill', normal, 39, 0, 8, 0, 0, 0),
    ('lhs_before_zero', '{p0[p1]=p0[0]/p2;0}', 'homes', [5, -1, 0, *normal[3:]], 0, 103, 8, 0, 1, 0),
    ('rhs_zero', '{p0[p1]=p0[p1]/p2;0}', 'homes', [5, 0, 0, *normal[3:]], 0, 102, 8, 0, 1, 0),
    ('rhs_bounds', '{p0[p1]=p0[p1+8];0}', 'homes', normal, 0, 103, 8, 0, 1, 0),
    ('index_overflow', '{p0[p1+1]=p2;0}', 'homes', [5, 2147483647, *normal[2:]], 0, 101, 8, 0, 0, 1),
    ('rhs_overflow', '{p0[p1]=p2+1;0}', 'homes', [5, 0, 9223372036854775807, *normal[3:]], 0, 101, 8, 0, 0, 0),
    ('empty', '{p0[p1]=p2;0}', 'homes', normal, 0, 103, 0, 0, 0, 1),
    ('guarded', '{if p1>=0&&p1<p0.len{p0[p1]=p2};p0[p1]}', 'homes', normal, 11, 0, 8, 0, 0, 1),
    ('walk512', '{p0[p1]=-('+balanced(256)+');p0[p1]}', 'homes', [5, 0, 1, *normal[3:]], -256, 0, 8, 0, 0, 0),
    ('walk513', '{p0[p1]='+balanced(257)+';p0[p1]}', 'homes', [5, 0, 1, *normal[3:]], 257, 0, 8, 1, 0, 0),
]
# name, body, layout, args, ret, result, expected scratch8/9 bits by proofmode, exact words
probes = [
    ('register_read', '{p0[p1]}', 'homes', normal, 2, 3, {'off': 256, 'on': 256}, {'off': 5, 'on': 5}),
    ('proven_register_read', '{if p1>=0&&p1<p0.len{return p0[p1]};return 0}', 'homes', normal, 2, 3, {'off': 256, 'on': 256}, {'off': 5, 'on': 2}),
    ('nested', '{p0[p1]+p0[p1+1]+p0[p1+2]}', 'homes', normal, 2, 7, {'off': 256, 'on': 256}, {}),
    ('compare_and', '{p1>=0&&p0[p1]>p2}', 'homes', [5, 0, 0, *normal[3:]], 3, 1, {'off': 256, 'on': 256}, {}),
    ('short_skip', '{p1>=0&&p0[p1]>p2}', 'homes', [5, -1, 0, *normal[3:]], 3, 0, {'off': 256, 'on': 256}, {}),
    ('constdiv', '{p2/3}', 'homes', normal, 2, 3, {'off': 0, 'on': 0}, {}),
    ('negative_constdiv', '{p2/(-3)}', 'homes', normal, 2, -3, {'off': 0, 'on': 0}, {}),
    ('dynamic_div', '{p2/p3}', 'homes', [5, 0, 11, 2, *normal[4:]], 2, 5, {'off': 0, 'on': 0}, {}),
    ('mul', '{p2*p3+p4}', 'homes', normal, 2, 160, {'off': 0, 'on': 0}, {}),
    ('read_div', '{(p0[p1]+p2)/3}', 'homes', normal, 2, 4, {'off': 256, 'on': 256}, {}),
    ('length', '{p0.len}', 'homes', normal, 1, 8, {'off': 0, 'on': 0}, {'off': 1, 'on': 1}),
    ('virtual_large_physical_small', '{p0[p1]}', 'mapped-small', normal, 2, 3, {'off': 256, 'on': 256}, {'off': 8, 'on': 8}),
    ('large_data', '{p0[p1]}', 'mapped-data', normal, 2, 3, {'off': 768, 'on': 768}, {'off': 10, 'on': 10}),
    ('large_index', '{p0[p1]}', 'mapped-index', normal, 2, 3, {'off': 768, 'on': 768}, {'off': 10, 'on': 10}),
    ('large_scalar', '{p2}', 'mapped-value', normal, 2, 11, {'off': 512, 'on': 512}, {'off': 3, 'on': 3}),
    ('large_length', '{p0.len}', 'mapped-length', normal, 1, 8, {'off': 512, 'on': 512}, {'off': 3, 'on': 3}),
    ('dead_length_spill', '{if p1>=0&&p1<p0.len{return p0[p1]};return 0}', 'mapped-length', normal, 2, 3, {'off': 768, 'on': 256}, {'off': 10, 'on': 4}),
]

with tempfile.TemporaryDirectory(prefix='dunder-array-addresses-') as directory, wasm_host.WasmWorker() as wasm:
    work = Path(directory)
    harness = work / 'emitter'
    built = subprocess.run([compiler, 'build', ROOT / 'tests/fixtures/array-address-emitter.ddr', '-o', harness], capture_output=True, timeout=60)
    assert built.returncode == 0, built.stderr.decode(errors='replace')

    def generate(name, body, target, proof, mode, layout, arguments, ret, expected, status, length=8):
        source = work / f'{name}.ddr'
        source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 ' + body)
        image = work / f'{name}-{target}-{proof}-{mode}'
        command = [harness, source, image, str(target), str(expected), str(ret), '21', *map(str, arguments), str(length), mode, layout, proof]
        result = subprocess.run(command, capture_output=True, timeout=15)
        assert result.returncode == 0, (name, target, proof, result.returncode, result.stdout, result.stderr)
        fields = list(map(int, result.stdout.splitlines()))
        assert len(fields) == 8, (name, fields)
        if target == host:
            run = subprocess.run([image], capture_output=True, timeout=10)
            assert run.returncode == status, (name, target, proof, run.returncode, status, run.stderr)
        if target == 3:
            run = wasm.run(image)
            assert run.returncode == (125 if status else 0), (name, target, proof, run.returncode, status, run.stderr)
        return fields

    for name, body, layout, arguments, expected, status, length, pushes, addr9, direct in stores:
        for target in range(4):
            for proof in ('off', 'on'):
                fields = generate(name, body, target, proof, 'function', layout, arguments, 2, expected, status, length)
                assert (fields[2], fields[4], fields[5]) == (pushes, addr9, direct), (name, target, proof, fields)
        print('PASS store', name, 'exact address/preservation words; four targets; native/Wasm executed', flush=True)
    for name, body, layout, arguments, ret, expected, masks, words in probes:
        for target in range(4):
            for proof in ('off', 'on'):
                fields = generate(name, body, target, proof, 'probe', layout, arguments, ret, expected, 0)
                assert fields[1] & 768 == masks[proof], (name, target, proof, fields)
                if proof in words:
                    assert fields[0] == words[proof], (name, target, proof, fields, words[proof])
                if not masks[proof] & 256:
                    assert fields[6] == 0, (name, 'unreported x8 write', fields)
                if not masks[proof] & 512:
                    assert fields[7] == 0, (name, 'unreported x9 write', fields)
        print('PASS probe', name, 'actual x8/x9 sentinels and write words; four targets', flush=True)
    source = work / 'unknown.ddr'
    source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 {p2}')
    for mode in ('unknown', 'depth'):
        command = [harness, source, work / 'unused', '0', '11', '2', '21', *map(str, normal), '8', mode, 'homes', 'off']
        result = subprocess.run(command, capture_output=True, timeout=10)
        assert result.returncode == 0 and result.stdout == b'-1\n', (mode, result.returncode, result.stdout, result.stderr)
        print('PASS conservative', mode, flush=True)
print(f'{len(stores)} store cases + {len(probes)} scratch probes; all four target images; native/Wasm executed')
