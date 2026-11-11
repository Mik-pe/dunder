#!/usr/bin/env python3
"""Verify actual bounds removal and trap order for the range proof emitter."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('compiler', type=Path)
args = parser.parse_args()
compiler = args.compiler.resolve(strict=True)
host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
params = [5, 0, 11, 13, 17, 19, 23, 29]
weighted = '+'.join(f'p0[{i}]*{i+1}' for i in range(8))
cases = [
    ('sum', '{var cursor:=0;var total:i64=0;while cursor<p0.len{total=total+p0[cursor];cursor=cursor+1};total}', 20, params, 42, 0, 8, 1, 0),
    ('insertion', '{for cursor in 1..p0.len{item:=p0[cursor];var hole:=cursor;while hole>0&&p0[hole-1]>item{p0[hole]=p0[hole-1];hole=hole-1};p0[hole]=item};'+weighted+'}', 21, params, 311, 0, 8, 3, 2),
    ('reverse', '{var left:=0;var right:=p0.len-1;while left<right{item:=p0[left];p0[left]=p0[right];p0[right]=item;left=left+1;right=right-1};'+weighted+'}', 21, params, 159, 0, 8, 2, 2),
    ('increment', '{for cursor in 0..p0.len{p0[cursor]=p0[cursor]+1};var total:i64=0;for cursor in 0..p0.len{total=total+p0[cursor]};total}', 21, params, 50, 0, 8, 2, 1),
    ('guard_join', '{if p1>=0&&p1<p0.len{p0[p1]=p0[p1]+1};p0[p1]}', 21, params, 4, 0, 8, 1, 1),
    ('early_return', '{if p1<0||p1>=p0.len{return 0};p0[p1]}', 21, [5, -1, *params[2:]], 0, 0, 8, 1, 0),
    ('lhs_first', '{p0[p1]=p0[0]/p2;0}', 21, [5, -1, 0, *params[3:]], 0, 103, 8, 0, 0),
    ('rhs_zero', '{if p1>=0&&p1<p0.len{p0[p1]=p0[p1]/p2};0}', 21, [5, 0, 0, *params[3:]], 0, 102, 8, 1, 1),
    ('rhs_bounds', '{if p1>=0&&p1<p0.len{p0[p1]=p0[p1+8]};0}', 21, params, 0, 103, 8, 0, 1),
    ('shortcircuit_order', '{if p0[p1]>0&&p1>=0&&p1<p0.len{p0[p1]=0};0}', 21, [5, -1, *params[2:]], 0, 103, 8, 0, 1),
    ('unsafe_offset', '{for cursor in 0..p0.len{p0[cursor+1]=0};0}', 21, params, 0, 103, 8, 0, 0),
    ('index_overflow', '{p0[p1+1]=0;0}', 21, [5, 2147483647, *params[2:]], 0, 101, 8, 0, 0),
    ('snapshot', '{var high:=p0.len;for cursor in 0..high{high=0;p0[cursor]=0};for cursor in 0..p0.len{if p0[cursor]!=0{return 1}};0}', 21, params, 0, 0, 8, 1, 1),
    ('break_exit', '{for cursor in 0..p0.len{var target:=cursor;while target>0{target=target-1;if p1>0{target=-1;break};p0[target]=0};p0[target]=0};0}', 21, [5, 1, *params[2:]], 0, 103, 8, 0, 1),
    ('empty', '{for cursor in 0..p0.len{p0[cursor]=p0[cursor]+1};0}', 21, params, 0, 0, 0, 1, 1),
    ('negative_backedge', '{var cursor:=0;while cursor<p0.len{p0[cursor]=0;cursor=-1};0}', 21, params, 0, 103, 8, 0, 0),
]

with tempfile.TemporaryDirectory(prefix='dunder-range-analysis-') as directory:
    work = Path(directory)
    harness = work / 'emitter'
    build = subprocess.run([compiler, 'build', ROOT / 'tests/fixtures/range-emitter.ddr', '-o', harness], capture_output=True, timeout=60)
    assert build.returncode == 0, build.stderr.decode(errors='replace')
    for name, body, first, arguments, expected, status, length, reads, stores in cases:
        source = work / f'{name}.ddr'
        source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 ' + body)
        for target in range(4):
            images, counters = {}, {}
            for mode in ('none', 'range', 'budget'):
                image = work / f'{name}-{target}-{mode}'
                proof_counts = (reads, stores) if mode == 'range' else (0, 0)
                command = [harness, source, image, str(target), str(expected), '2', str(first), *map(str, arguments), str(length), mode, *map(str, proof_counts)]
                result = subprocess.run(command, capture_output=True, timeout=10)
                assert result.returncode == 0, (name, target, mode, result.returncode, result.stdout, result.stderr)
                counters[mode] = tuple(map(int, result.stdout.splitlines()))
                assert len(counters[mode]) == 3
                images[mode] = image.read_bytes()
                if target == host:
                    run = subprocess.run([image], capture_output=True, timeout=10)
                    assert run.returncode == status, (name, mode, run.returncode, status, run.stderr)
                if target == 3:
                    run = subprocess.run(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], capture_output=True, timeout=10)
                    assert run.returncode == (125 if status else 0), (name, mode, run.returncode, status, run.stderr)
            assert counters['none'][0] - counters['range'][0] == reads + stores, (name, target, counters)
            assert counters['none'][1:] == counters['range'][1:], (name, target, 'arithmetic/zero checks changed', counters)
            assert images['none'] == images['budget'], (name, target, 'budget fallback changed code')
            assert (images['none'] != images['range']) == (reads + stores > 0), (name, target, 'proofs did not change emitted code')
        print('PASS', name, 'all four targets; native/Wasm executed; exact bounds removal; arithmetic unchanged', flush=True)
    for mode in ('idle', 'twice', 'shape', 'budget-apply'):
        source = work / 'guard.ddr'
        source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 {for cursor in 0..p0.len{p0[cursor]=0};0}')
        command = [harness, source, work / 'unused', str(host), '0', '2', '21', *map(str, params), '8', mode, '0', '0']
        result = subprocess.run(command, capture_output=True, timeout=10)
        assert result.returncode == 104, (mode, result.returncode, result.stdout, result.stderr)
        print('PASS', mode, 'rejected', flush=True)
print(f'{len(cases)} runtime cases verified; invalid apply lifecycles rejected')
