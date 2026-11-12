#!/usr/bin/env python3
"""Exact-bit SSA differential QA through real target lowerers, without timings.

Physical canonical MIR is interpreted independently of retained words, then
the actual before/after images execute on the native host and Wasm. Calls/FP
and unknown scratch/exit observations are explicit interpreter gaps with
native/Wasm helper coverage. Positive cell
cases must actually remove loads; opaque/escape/overlap cases never stand in
for an optimization success. Failed budgets must restore EVERY Function field.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('ssa_mir_semantic_oracle', ROOT / 'tools/test-mir.py')
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)

MASK = (1 << 64) - 1
LOW = (1 << 32) - 1
DEFAULT_LIMIT = 4194304
COUNTERS = ('changed', 'status', 'units', 'cells', 'removed_loads', 'removed_stores',
            'copies', 'parameters', 'values', 'registers', 'removed_pushes', 'removed_pops', 'words')
NAMES = ('zero_w', 'sign_w', 'byte_view', 'high_w', 'counted_loop', 'expression_stack',
         'critical_edge_flags', 'loop_swap', 'partial_store', 'escaped_cell', 'old_call',
         'fp_transfer', 'many_values', 'entry_flags', 'checked_overflow', 'pressure', 'late_initializer',
         'external_jump', 'external_conditional', 'external_fallthrough', 'half_view', 'combined_views', 'call_local',
         'unreachable_external')
POSITIVE = {0, 1, 2, 3, 4, 6, 7, 12, 13, 15, 16, 20, 21, 22, 23}


def digest(value):
    return hashlib.sha256(value).hexdigest()


def seed(reg):
    return 123456789012345 + reg * 1000003


def overflow_sub(a, b):
    return not -(1 << 63) <= a - b < (1 << 63)


def expected(case, value, control):
    if case == 0: result = value & LOW
    elif case == 1: result = oracle.signed(value, 32)
    elif case == 2: result = (value >> 8) & 255
    elif case == 3: result = (value & MASK) >> 32
    elif case in (4, 16): result = value * (value - 1) // 2
    elif case == 5: result = value - 10
    elif case == 6: result = value + 1 if control == 0 else (value & LOW) + (value == 0)
    elif case == 7: result = 51 + value if control % 2 else value * 3 + 17
    elif case == 8: result = (value & 0xFFFFFFFF00000000) | (control & LOW)
    elif case in (9, 11, 17, 18, 19, 23): result = value
    elif case in (10, 22): result = value + 99
    elif case == 12: result = value + sum(range(1, 81))
    elif case == 13: result = int(overflow_sub(value, control))
    elif case == 14: result = (value + 1) & LOW
    elif case == 15: result = 2 * (value * 40 + sum(range(40)))
    elif case == 20: result = (value >> 24) & 65535
    elif case == 21: result = ((value >> 8) & 255) + ((value >> 24) & 65535) + (value & LOW) + oracle.signed(value, 32) + value
    else: raise AssertionError(case)
    return oracle.signed(result)


def corpus():
    pattern = oracle.signed(0xFEDCBA98800000FF)
    for case in range(len(NAMES)):
        if case in (0, 1, 2, 3, 20, 21): inputs = [(pattern, 0), (0x12345678FFFFFFFF, 1), (0, 0)]
        elif case in (4, 16): inputs = [(0, 0), (1, 0), (7, 0), (23, 0)]
        elif case == 6: inputs = [(pattern, 0), (pattern, 1), (0, 1), (0, -1)]
        elif case == 7: inputs = [(13, 0), (13, 1), (13, 2), (-7, 5)]
        elif case == 8: inputs = [(0x1122334455667788, 0xAABBCCDD), (-1, 0)]
        elif case == 11: inputs = [(0x7FF8000000000042, 0), (pattern, 0)]
        elif case == 13: inputs = [(-(1 << 63), 1), ((1 << 63) - 1, -1), (0, 0)]
        elif case == 14: inputs = [((1 << 63) - 1, 0), ((1 << 63) - 2, 0), (-1, 0)]
        else: inputs = [(41, 0), (-17, 1)]
        for value, control in inputs:
            yield case, value, control


def parse_export(text):
    original, result = text.split('ORIGINAL\n', 1)[1].split('RESULT\n', 1)
    result, raw_counters = result.split('COUNTERS\n', 1)

    def phase(value):
        model, metadata = value.split('META1\n', 1)
        metadata, rest = metadata.split('ENDMETA\n', 1)
        assert not rest.strip(), rest
        return oracle.decode_model(model), metadata

    before, before_meta = phase(original)
    after, after_meta = phase(result)
    rows = raw_counters.splitlines()
    assert len(rows) == len(COUNTERS), (rows, COUNTERS)
    counters = dict(zip(COUNTERS, [rows[0] == 'true', *map(int, rows[1:])]))
    assert rows[0] in ('true', 'false')
    counters['before_words'] = int(before_meta.splitlines()[9]) - int(before_meta.splitlines()[5])
    return before, after, before_meta, after_meta, counters


def traffic(model):
    counts = {'loads': 0, 'stores': 0, 'pushes': 0, 'pops': 0}
    for op in model['operations']:
        if op['mode'] == 2: continue
        for key, kind in (('loads', 7), ('stores', 8), ('pushes', 10), ('pops', 11)):
            counts[key] += op['kind'] == kind
        assert all(-1 <= op[key] <= 63 for key in ('dst', 'a', 'b', 'c')), op
    return counts


def interpret(model, case, value, control, trap):
    if case == 15:
        # This case deliberately consumes X9 after a retained helper's unknown
        # clobber. Its actual address bits are not a canonical SSA definition:
        # the generic physical interpreter cannot manufacture that meaning.
        # Exact native/Wasm execution observes those bits instead.
        return {'coverage_gap': 'consumed retained-helper scratch/clobber result'}
    # Address is an explicit semantic base+byte-offset operation. Extend the
    # existing interpreter's closed vocabulary without decoding retained bits.
    operations = []
    for op in model['operations']:
        if op['kind'] == 9 and op['mode'] != 2:
            assert op['type'] == 2 and op['a'] >= 0 and op['b'] == op['c'] == -1 and not op['flags'] and not op['clobbers'], op
            op = dict(op, kind=16, opcode=1, aux=0)
        operations.append(op)
    machine = oracle.Interpreter(dict(model, operations=operations), fuel=250000)
    machine.variables.update({reg: seed(reg) & MASK for reg in range(31)})
    machine.variables.update({4: value & MASK, 5: control & MASK, 31: 1 << 20})
    difference = (value - control) & MASK
    machine.nzcv = (bool(difference >> 63), difference == 0,
                    (value & MASK) >= (control & MASK), overflow_sub(value, control))
    try:
        result = oracle.signed(machine.run())
    except oracle.CoverageGap as gap:
        message = str(gap)
        if case in (10, 22): assert 'kind=14' in message, message
        elif case == 11: assert 'kind=19' in message, message
        elif case in (17, 18): assert 'external branch' in message, (case, message)
        elif case == 19: assert 'fell off end' in message, (case, message)
        else: assert trap and 'external branch' in message, (case, trap, message)
        return {'coverage_gap': message, 'steps': machine.steps}
    assert not trap and result == expected(case, value, control), (NAMES[case], value, control, result)
    for reg in range(18, 31):
        assert machine.read(reg) == seed(reg) & MASK, (NAMES[case], reg, machine.read(reg), seed(reg))
    assert machine.read(31) == 1 << 20, (NAMES[case], machine.read(31))
    return {'result': result, 'steps': machine.steps, 'coverage': dict(machine.coverage)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--case', type=int, choices=range(len(NAMES)))
    parser.add_argument('--report', type=Path)
    parser.add_argument('--output', type=Path, help='empty directory to keep images and raw exports')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    module = ROOT / 'selfhost/mir_ssa.ddr'
    assert module.is_file(), 'production mir_ssa.ddr must be integrated before building the QA consumer'
    fixture = ROOT / 'tests/fixtures/mir-ssa-oracle.ddr'
    # Refuse a stale rollback witness when the production schema grows.
    declaration = (ROOT / 'selfhost/mir.ddr').read_text().split('pub struct Function {', 1)[1].split('\n}', 1)[0]
    witness = fixture.read_text().split('fn metadata(', 1)[1].split('\nfn main()', 1)[0]
    fields = set(re.findall(r'\b([a-z][a-z0-9_]*)\s*:', declaration))
    observed = set(re.findall(r'\bf\.([a-z][a-z0-9_]*)', witness))
    assert fields == observed, ('incomplete Function rollback witness', fields - observed, observed - fields)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    report = dict(compiler_sha256=digest(compiler.read_bytes()), module_sha256=digest(module.read_bytes()),
                  fixture_sha256=digest(fixture.read_bytes()),
                  oracle_sha256=digest((ROOT / 'tools/test-mir.py').read_bytes()), host_target=host, images=[])
    with tempfile.TemporaryDirectory(prefix='dunder-mir-ssa-') as temporary:
        work = args.output.resolve() if args.output else Path(temporary)
        if args.output:
            work.mkdir(parents=True, exist_ok=True)
            assert not any(work.iterdir()), 'output directory must be empty'
        emitter = work / 'emitter'

        def call(command, status=0):
            result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True, text=True, timeout=60)
            assert result.returncode == status, (command, result.returncode, result.stdout[-8000:], result.stderr)
            return result

        call([compiler, 'build', ROOT / 'tests/fixtures/mir-ssa-oracle.ddr', '-o', emitter])
        report['emitter_sha256'] = digest(emitter.read_bytes())
        executed = proved = gaps = 0
        samples = [row for row in corpus() if args.case is None or row[0] == args.case]

        def one(case, value, control, target, mode, limit=DEFAULT_LIMIT, source=0, terminals=0, label=''):
            nonlocal executed, proved, gaps
            image = work / f'{case}-{value}-{control}-{target}-{mode}-{limit}-{source}-{terminals}{label}'
            result = call([emitter, image, target, case, mode, value, control,
                           expected(case, value, control), limit, source, terminals])
            before, after, bm, am, counters = parse_export(result.stdout)
            assert 0 <= counters['units'] <= max(0, limit), counters
            # This pass's published first-version contract is write-through:
            # keep original stores (including timing/order/extra GP effects).
            for index, op in enumerate(before['operations']):
                if op['kind'] == 8 and op['mode'] != 2:
                    assert after['operations'][index] == op, ('source store changed', NAMES[case], index)
            trap = case == 14 and value == (1 << 63) - 1
            pre = interpret(before, case, value, control, trap)
            post = interpret(after, case, value, control, trap)
            assert ('coverage_gap' in pre) == ('coverage_gap' in post)
            if 'coverage_gap' in pre: gaps += 1
            else: proved += 1; assert pre['result'] == post['result']
            before_traffic, after_traffic = traffic(before), traffic(after)
            if mode == 'off' or not counters['changed']:
                assert bm == am and before == after, (NAMES[case], counters, 'noncommitted MIR changed')
            if mode == 'off':
                assert counters['status'] == 0 and counters['units'] == 0
            elif limit == DEFAULT_LIMIT and (case in POSITIVE or (case == 14 and terminals)):
                assert counters['changed'] and counters['status'] == 2, (NAMES[case], counters)
                assert counters['cells'] > 0 and counters['removed_loads'] > 0, (NAMES[case], counters)
                assert after_traffic['loads'] < before_traffic['loads'], (NAMES[case], before_traffic, after_traffic, counters)
                if case == 12: assert counters['values'] > 64, counters
                if case in (4, 6, 7, 16): assert counters['parameters'] > 0, counters
                assert counters['registers'] > 0, counters
            if mode == 'on' and limit in (0, 1):
                assert not counters['changed'] and counters['status'] == -3, counters
            if mode == 'on' and (case in (17, 18, 19) or (case == 14 and not terminals)):
                assert not counters['changed'] and counters['status'] == -2, counters
            ran = False
            if target == host:
                call([image], 101 if trap else 0); executed += 1; ran = True
            elif target == 3:
                call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], 125 if trap else 0)
                executed += 1; ran = True
            if args.output: image.with_suffix(image.suffix + '.mir.txt').write_text(result.stdout)
            row = dict(case=case, name=NAMES[case], input=value, control=control, target=target, mode=mode,
                       limit=limit, source_objects=source, terminal_contract=terminals, executed=ran,
                       sha256=digest(image.read_bytes()), export_sha256=digest(result.stdout.encode()),
                       counters=counters, before_traffic=before_traffic, after_traffic=after_traffic,
                       before=pre, after=post)
            report['images'].append(row)
            return counters

        for index, (case, value, control) in enumerate(samples):
            for target in range(4):
                one(case, value, control, target, 'off')
                one(case, value, control, target, 'on')
                if case == 14: one(case, value, control, target, 'on', terminals=1)
            print(f'PASS {index + 1}/{len(samples)} {NAMES[case]} input{value} control{control}', flush=True)
        # Independent policy paths and exact rollback after early/mid/late budget
        # exhaustion. Reusing the production exclusive API is mandatory here.
        for case, value, control in ((0, 41, 0), (4, 7, 0), (12, -17, 1)):
            if args.case is not None and args.case != case: continue
            for target in (host, 3):
                full = one(case, value, control, target, 'on', source=1, label='-context')
                for limit in sorted({0, 1, max(1, full['units'] // 2), max(1, full['units'] - 1)}):
                    stopped = one(case, value, control, target, 'on', limit=limit, source=1, label='-budget')
                    assert not stopped['changed'] and stopped['status'] == -3, (case, limit, full, stopped)
        report.update(images_built=len(report['images']), executions=executed,
                      independent_pairs=proved, explicit_gap_pairs=gaps,
                      coverage_limits=['calls, FP, consumed unknown scratch and external exits are interpreter gaps; actual observing images execute',
                                       'Linux images are built but not executed on an ARM64/macOS host',
                                       'expression-stack case is semantic coverage without a promised stack optimization'])
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'{report["images_built"]} images; {executed} native/Wasm executions; '
          f'{proved} independent semantic pairs; {gaps} explicit semantic gaps; no latency measurements')


if __name__ == '__main__':
    main()
