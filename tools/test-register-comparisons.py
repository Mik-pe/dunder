#!/usr/bin/env python3
"""Check comparison instruction placement and signed/trapping semantics."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import wasm_host
RELATIONS = {'==': (0, lambda a, b: a == b), '!=': (1, lambda a, b: a != b),
             '<': (11, lambda a, b: a < b), '<=': (13, lambda a, b: a <= b),
             '>': (12, lambda a, b: a > b), '>=': (10, lambda a, b: a >= b)}


def cmp(left, right):
    return 0xEB00001F + (right << 16) + (left << 5)


def cmp_imm(reg, value):
    shift = 0 if value <= 4095 else 1
    imm = value if not shift else value // 4096
    return 0xF100001F + (shift << 22) + (imm << 10) + (reg << 5)


def cset(relation):
    return 0x9A9F07E0 + ((RELATIONS[relation][0] ^ 1) << 12)


def materialize(reg, value):
    value &= (1 << 64) - 1
    words = [0xD2800000 + ((value & 65535) << 5) + reg]
    for part in range(1, 4):
        bits = (value >> (part * 16)) & 65535
        if bits:
            words.append(0xF2800000 + (part << 21) + (bits << 5) + reg)
    return words


def frame_load(slot, reg):
    if slot < 32:
        return [4164944800 + ((-(slot + 1) * 8) & 511) * 4096 + reg]
    address = ([3506439081 + (slot + 1) * 8192] if slot < 511 else
               materialize(9, (slot + 1) * 8) + [3406365609])
    return address + [4181721376 + reg]


def call(command, status=0):
    result = subprocess.run([str(x) for x in command], capture_output=True,
                            text=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, status, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-register-compare-') as directory:
        work = Path(directory)
        harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/comparison-emitter.ddr', '-o', harness])
        count = 0
        queued = []

        def probe(body, left=3, right=7, zero=1, scalar=2, mode='homes', expected=0,
                  status=0, words=None, ret=3):
            nonlocal count
            count += 1
            queued.append((count, body, left, right, zero, scalar, mode, expected, status, words, ret))

        # Exact words prove direct allocated-register use, one-sided reloads,
        # noncontiguous/high physical slots and the cached array-length home.
        probe('{a<b}', expected=1, words=[cmp(4, 10), cset('<')])
        probe('{a<=b}', mode='left-spill', expected=1,
              words=frame_load(37, 0) + [cmp(0, 10), cset('<=')])
        probe('{a>=b}', mode='right-spill', expected=0,
              words=frame_load(620, 0) + [cmp(4, 0), cset('>=')])
        probe('{a!=b}', mode='spills', expected=1,
              words=frame_load(620, 0) + frame_load(37, 1) + [cmp(1, 0), cset('!=')])
        probe('{values.len>a}', scalar=1, expected=1, words=[cmp(11, 4), cset('>')])
        probe('{a<values.len}', scalar=1, mode='spills', expected=1,
              words=frame_load(113, 0) + frame_load(37, 1) + [cmp(1, 0), cset('<')])
        for immediate in (0, 1, 4095, 4096, 8192, 16773120):
            probe(f'{{a<{immediate}}}', expected=int(3 < immediate),
                  words=[cmp_imm(4, immediate), cset('<')])
            probe(f'{{a<={immediate}}}', mode='swapped', expected=int(immediate <= 3),
                  words=[cmp_imm(4, immediate), cset('>=')])
        probe('{a<4095}', mode='left-spill', expected=1,
              words=frame_load(37, 0) + [cmp_imm(0, 4095), cset('<')])
        for constant in (-1, -4096, 4097, 16777216, 9223372036854775807):
            probe(f'{{a<{constant}}}', expected=int(3 < constant),
                  words=materialize(0, constant) + [cmp(4, 0), cset('<')])

        for scalar, low, high in ((1, -(1 << 31), (1 << 31) - 1),
                                  (2, -(1 << 63), (1 << 63) - 1)):
            for left, right in ((low, high), (high, low), (low, low), (-1, 0)):
                for relation, (_, oracle) in RELATIONS.items():
                    expected = int(oracle(left, right))
                    for mode in ('homes', 'spills'):
                        probe(f'{{a{relation}b}}', left, right, scalar=scalar, mode=mode, expected=expected)
                    probe(f'{{if a{relation}b{{return 1}};0}}', left, right,
                          scalar=scalar, mode='function', expected=expected, ret=2)
                    probe(f'{{if !(a{relation}b){{return 0}};1}}', left, right,
                          scalar=scalar, mode='function', expected=expected, ret=2)

        cases = [
            ('{a/z<b}', 4, 7, 0, 2, 0, 102),
            ('{b<a/z}', 4, 7, 0, 2, 0, 102),
            ('{(a+1)<(b/z)}', (1 << 63) - 1, 7, 0, 2, 0, 101),
            ('{(a/z)<(b+1)}', 4, (1 << 63) - 1, 0, 2, 0, 102),
            ('{0<a+1}', (1 << 31) - 1, 7, 1, 1, 0, 101),
            ('{a+1<0}', (1 << 63) - 1, 7, 1, 2, 0, 101),
            ('{a==0&&b/z>0}', 1, 7, 0, 2, 0, 0),
            ('{a!=0||b/z>0}', 1, 7, 0, 2, 1, 0),
            ('{a==0&&b/z>0}', 0, 7, 0, 2, 0, 102),
            ('{values[a]<values[b]/0}', 8, 0, 1, 1, 0, 103),
            ('{values[a]<values[b]/0}', 0, 0, 1, 1, 0, 102),
            ('{a/z<0}', 14, 7, 2, 2, 0, 0),
            ('{0<a/z}', 14, 7, 2, 1, 1, 0),
        ]
        for body, left, right, zero, scalar, expected, status in cases:
            for mode in ('homes', 'spills'):
                probe(body, left, right, zero, scalar, mode, expected, status)
        for left, right in ((0, 0), (0, 1), (1, 0), (1, 1)):
            probe('{a&&b}', left, right, scalar=3, expected=int(bool(left and right)))
            probe('{a||b}', left, right, scalar=3, expected=int(bool(left or right)))
        for relation, (_, oracle) in RELATIONS.items():
            for value in (-1, 0, 1):
                expected = int(oracle(0, value))
                probe(f'{{0{relation}a}}', value, scalar=1, expected=expected)
                probe(f'{{if 0{relation}a{{return 1}};0}}', value,
                      scalar=1, mode='function', ret=2, expected=expected)
        probe('{var sum:=0;for cursor in 0..a{if cursor<b{sum=sum+1}};sum}',
              6, 3, scalar=1, mode='function', ret=1, expected=3)
        with wasm_host.WasmWorker() as wasm, ThreadPoolExecutor(max_workers=4) as pool:
            def run_probe(item):
                cid, body, left, right, zero, scalar, mode, expected, status, words, ret = item
                source = work / f'case-{cid}.ddr'
                source.write_text('a b z values ' + body)
                for target in range(4):
                    image = work / f'image-{cid}-{target}'
                    generated = call([harness, source, image, target, mode, expected, ret,
                                      scalar, left, right, zero, 8])
                    emitted = list(map(int, generated.stdout.splitlines()))
                    if words is not None:
                        assert emitted == words, (body, target, mode, emitted, words)
                    if target == host:
                        run = call([image], status)
                        assert run.stdout == '', (body, run.stdout)
                    elif target == 3:
                        expected_wasm = 125 if status else 0
                        wres = wasm.run(image)
                        assert wres.returncode == expected_wasm and wres.stdout == '', (
                            body, wres.returncode, expected_wasm, wres.stdout, wres.stderr)
                    image.unlink(missing_ok=True)
            list(pool.map(run_probe, queued))
        print(f'PASS {count} comparison cases: exact words, signed results, branches, traps and shortcuts; '
              'all four targets built, host and Wasm executed; cross-native not executed')


if __name__ == '__main__':
    main()
