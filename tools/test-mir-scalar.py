#!/usr/bin/env python3
"""Independent scalar MIR results/NZCV on native host and Wasm; four image targets.

This exercises the inactive canonical-scalar modules directly. It does not enable
new production dispatch, patch retained words, or claim cross-target execution.
The existing semantic MIR oracle supplies integer arithmetic and NZCV results;
it reads semantic fields and never knows the new lowering's instruction words.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
from pathlib import Path
import platform
import random
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / 'tools'))
import wasm_host
spec = importlib.util.spec_from_file_location('semantic_mir_oracle', ROOT / 'tools/test-mir.py')
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def op(kind, opcode, ty, **changes):
    result = dict(kind=kind, opcode=opcode, type=ty, dst=4, a=1, b=-1, c=-1,
                  imm=0, aux=0, flags=0, effects=0, mode=1)
    if kind in (4, 5):
        result['b'] = 2
    if opcode in (17, 18):
        result['c'] = 3
    if kind == 5:
        result.update(dst=-1, flags=2)
    if kind == 6:
        result.update(a=-1, flags=1)
    result.update(changes)
    return result


def evaluate(operation, values):
    ret = dict(kind=15, opcode=1, mode=0, type=2)
    model = dict(operations=[operation, ret], order=[0, 1], labels=[], places=[])
    machine = oracle.Interpreter(model)
    machine.variables.update({0: 0, 1: values[0] & oracle.MASK64,
                              2: values[1] & oracle.MASK64, 3: values[2] & oracle.MASK64,
                              4: 1084818905618843912})
    if operation['kind'] == 6:
        machine.binary(op(5, 2, operation['type']), 32 if operation['type'] == 1 else 64)
    machine.run()
    result = machine.read(operation['dst']) if operation['dst'] >= 0 else 0
    conditions = 0
    if operation['flags'] == 2:
        conditions = sum(int(machine.condition(condition)) << condition for condition in range(14))
    return oracle.signed(result), conditions


def cases():
    rng = random.Random(0xD00D2026)
    result = []
    for ty, width in ((1, 32), (2, 64)):
        pairs = [(7, 3), (-1, 0), (-(1 << (width - 1)), -1),
                 (rng.getrandbits(64), rng.getrandbits(64))]
        for opcode in range(1, 19):
            if ty == 1 and opcode in (4, 5):
                continue
            for left, right in pairs:
                result.append((f'binary-{width}-{opcode}', op(4, opcode, ty), (left, right, -17)))
        for opcode in (1, 2, 8, 11):
            for left, right in pairs:
                result.append((f'nzcv-{width}-{opcode}', op(5, opcode, ty), (left, right, 0)))
        for opcode in (1, 2, 8, 9, 10, 11, 12, 13):
            for shift_kind in range(4 if opcode >= 8 else 3):
                result.append((f'shifted-{width}-{opcode}-{shift_kind}',
                               op(4, opcode, ty, aux=(width - 1) | (shift_kind << 6)),
                               (-4611686018427387905, 0x8000000180000003, 0)))
        for opcode in (1, 2):
            for immediate in (1, 4095, 4096, 4095 * 4096):
                result.append((f'immediate-{width}-{opcode}-{immediate}',
                               op(4, opcode, ty, b=-1, imm=immediate, flags=2), (-1, 0, 0)))
        for opcode in (1, 2):
            for rotate, top in ((0, width - 1), (7, 15), (width - 1, width - 1)):
                result.append((f'bitfield-{width}-{opcode}-{rotate}-{top}',
                               op(3, opcode, ty, aux=rotate | (top << 6)), (-9223372034707292161, 0, 0)))
        result.append((f'unsigned-wrapping-field-{width}', op(3, 2, ty, aux=13 | (5 << 6)), (-1, 0, 0)))
        for shift in range(0, width, 16):
            for dst in (1, 4):
                result.append((f'insert-{width}-{shift}-dst{dst}',
                               op(3, 3, ty, dst=dst, aux=shift, imm=oracle.signed(0xBEEF << shift)),
                               (-4919131752989213765, 0, 0)))
        for condition in range(14):
            for left, right in ((7, 3), (-1, 0), (0, 0)):
                result.append((f'select-{width}-{condition}', op(6, 0, ty, imm=condition), (left, right, 0)))
        for source in ('left-zero', 'right-zero', 'both-zero'):
            result.append((f'zr-{width}-{source}',
                           op(4, 1, ty, a=-1 if source != 'right-zero' else 1,
                              b=-1 if source != 'left-zero' else 2), (91, 137, 0)))
        for dst in (1, 2, 3):
            result.append((f'alias-dst-{width}-{dst}', op(4, 18, ty, dst=dst), (91, 137, -17)))
    return result


def run(command, cwd):
    completed = subprocess.run(list(map(str, command)), cwd=cwd, capture_output=True,
                               text=True, timeout=60)
    assert completed.returncode == 0, (command, completed.returncode, completed.stdout, completed.stderr)
    return completed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--limit', type=int, help='development subset; omitted verifies the complete deterministic corpus')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    corpus = cases()
    if args.limit is not None:
        corpus = corpus[:args.limit]
    with tempfile.TemporaryDirectory(prefix='dunder-mir-scalar-') as temporary:
        work = Path(temporary); emitter = work / 'emitter'
        run([compiler, 'build', ROOT / 'tests/fixtures/mir-scalar-emitter.ddr', '-o', emitter], work)
        with wasm_host.WasmWorker() as wasm, ThreadPoolExecutor(max_workers=4) as pool:
            def check_case(entry):
                index, (_, operation, values) = entry
                expected, conditions = evaluate(operation, values)
                semantic = [operation[key] for key in ('kind', 'opcode', 'type', 'dst', 'a', 'b', 'c', 'imm', 'aux', 'flags')]
                for target in range(4):
                    image = work / f'scalar-image-{index}-{target}'
                    run([emitter, image, target, *semantic, *(oracle.signed(value) for value in values), expected, conditions], work)
                    if target == host:
                        run([image], work)
                    if target == 3:
                        wres = wasm.run(image)
                        assert wres.returncode == 0, (image, wres.returncode, wres.stdout, wres.stderr)
                    image.unlink(missing_ok=True)
            for done_count, _ in enumerate(pool.map(check_case, enumerate(corpus)), 1):
                if done_count % 50 == 0:
                    print(f'PASS {done_count}/{len(corpus)} scalar cases', flush=True)
    print(f'{len(corpus)} independent scalar/NZCV cases; {len(corpus) * 4} images built; '
          f'{len(corpus) * 2} native/Wasm executions passed')


if __name__ == '__main__':
    main()
