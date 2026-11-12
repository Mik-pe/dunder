#!/usr/bin/env python3
"""Regional frame SSA: exact helper semantics plus actual target observers.

No timings. An independent byte-memory interpreter expands the finite helper's
specified calls, checks its incoming GP/NZCV/SP and frame bytes, then models its
partial write and fresh clobbers. Native/Wasm execute the real helper instead.
Only the explicitly consumed large-helper scratch case remains an oracle gap.
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
spec = importlib.util.spec_from_file_location('region_existing_qa', ROOT / 'tools/test-mir-ssa.py')
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)
oracle = qa.oracle
MASK = (1 << 64) - 1
LOW = (1 << 32) - 1
LIMIT = 4194304
ENTRY_SP = 1 << 20
FAIL = -9999
NAMES = ('full_after_call', 'narrow_first', 'all_views', 'two_calls', 'asymmetric_call',
         'missing_init_arm', 'partial_init', 'zero_trip_loop', 'before_and_after',
         'postcall_flags', 'observed_large_scratch', 'unreachable_init', 'raw_alias', 'explicit_trap', 'large_capture')
NEGATIVE = {5, 6, 11}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected(case, value, control):
    bits = ((value & MASK) & ~LOW) | (control & LOW)
    if case in (0, 10, 14): result = bits
    elif case == 2: result = ((bits >> 8) & 255) + ((bits >> 24) & 65535) + (bits & LOW) + oracle.signed(bits, 32) + oracle.signed(bits)
    elif case in (3, 12): result = (control & LOW) + ((control + 1) & LOW)
    elif case == 4: result = (control if control else value) & LOW
    elif case == 7: result = max(control, 0) * (control & LOW)
    elif case == 8: result = (value & LOW) + (control & LOW)
    elif case == 9: result = (control & LOW) + 8 * qa.overflow_sub(value, control)
    else: result = control & LOW
    return oracle.signed(result)


def corpus():
    bits = oracle.signed(0xFEDCBA98800000FF)
    for case in range(len(NAMES)):
        if case == 7: values = [(41, 0), (-17, 1), (bits, 4), (41, 7)]
        elif case in (4, 5, 11, 13): values = [(bits, 0), (bits, 1)]
        elif case == 9: values = [(-(1 << 63), 1), ((1 << 63) - 1, -1), (0, 0)]
        else: values = [(bits, 0x800000FF), (0x12345678FFFFFFFF, 0xAABBCCDD)]
        for value, control in values: yield case, value, control


def instruction(kind, **fields):
    result = dict.fromkeys(oracle.COLUMNS, 0)
    result.update(kind=kind, dst=-1, a=-1, b=-1, c=-1, place=-1, target=-1, mode=1, type=2)
    result.update(fields)
    return result


def expand_helpers(model, helpers, case, value, control):
    """Interpret a declared finite helper, never guess unknown raw Call meaning."""
    operations = []
    boundaries = []
    places = [dict(p) for p in model['places']]

    def place(offset, width):
        for index, p in enumerate(places):
            if p == dict(kind=1, offset=offset, width=width): return index
        places.append(dict(kind=1, offset=offset, width=width))
        return len(places) - 1

    def constant(dst, bits): operations.append(instruction(1, dst=dst, imm=bits))

    def check(variable, bits):
        constant(100, bits)
        operations.append(instruction(5, opcode=2, a=variable, b=100, flags=2))
        operations.append(instruction(13, opcode=1, imm=1, flags=1, target=FAIL))

    for opid in model['order']:
        boundaries.append(len(operations))
        op = model['operations'][opid]
        if op['kind'] != 14 or op['mode'] == 2:
            operations.append(dict(op)); continue
        assert op['target'] in helpers[:2], op
        current = control + int(op['target'] == helpers[1])
        current = oracle.signed(current)
        # Snapshot flags before any observer comparison, as the actual helper.
        for index, condition in enumerate((0, 2, 4, 6)):
            operations.append(instruction(6, dst=101 + index, imm=condition, flags=1))
        for reg in range(18):
            wanted = qa.seed(reg)
            if reg == 0: wanted = ENTRY_SP + 32
            if reg == 4: wanted = value
            if reg == 5: wanted = current
            if reg == 7 and case == 12: wanted = ENTRY_SP - 24
            check(reg, wanted)
        a, b = (control, 0) if case in (4, 5) else (value, current)
        difference = (a - b) & MASK
        flags = (difference == 0, (a & MASK) >= (b & MASK), bool(difference >> 63), qa.overflow_sub(a, b))
        for index, flag in enumerate(flags): check(101 + index, int(flag))
        operations.append(instruction(16, opcode=1, dst=105, a=29, imm=-96))
        operations.append(instruction(5, opcode=2, a=31, b=105, flags=2))
        operations.append(instruction(13, opcode=1, imm=1, flags=1, target=FAIL))
        if case not in NEGATIVE:
            operations.append(instruction(7, dst=105, a=29, imm=-8, aux=8, place=place(-8, 8)))
            check(105, value)
        operations.append(instruction(8, a=5, b=29, imm=-8, aux=4, place=place(-8, 4), type=1))
        for reg in range(18): constant(reg, 9000007 + reg * 101)
        constant(16, value); constant(17, current)
        operations.append(instruction(5, opcode=2, a=16, b=17, flags=2))
        constant(0, 99)
    boundaries.append(len(operations))
    labels = [(label, boundaries[at] if at >= 0 else -1) for label, at in model['labels']]
    return dict(operations=operations, places=places, labels=labels, order=list(range(len(operations))))


def interpret(model, helpers, case, value, control):
    if case in (10, 14):
        return {'gap': 'consumed retained large-helper scratch; actual native/Wasm observer covers exact address bits'}
    machine = oracle.Interpreter(expand_helpers(model, helpers, case, value, control), fuel=250000)
    machine.variables.update({reg: qa.seed(reg) & MASK for reg in range(31)})
    machine.variables.update({0: ENTRY_SP + 32, 4: value & MASK, 5: control & MASK,
                              29: ENTRY_SP + 32, 31: ENTRY_SP})
    if case == 12: machine.variables[7] = ENTRY_SP - 24
    difference = (value - control) & MASK
    machine.nzcv = (bool(difference >> 63), difference == 0,
                    (value & MASK) >= (control & MASK), qa.overflow_sub(value, control))
    try: result = oracle.signed(machine.run())
    except oracle.CoverageGap as error:
        assert case == 13 and control == 0 and str(error) == f'external branch {helpers[2]}', (NAMES[case], error)
        return {'trap': 102, 'steps': machine.steps}
    assert not (case == 13 and control == 0)
    assert result == expected(case, value, control), (NAMES[case], value, control, result)
    assert machine.read(31) == ENTRY_SP and machine.read(29) == ENTRY_SP + 32
    for reg in range(18, 29): assert machine.read(reg) == qa.seed(reg), (NAMES[case], reg, machine.read(reg))
    return {'result': result, 'steps': machine.steps, 'coverage': dict(machine.coverage)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--case', type=int, choices=range(len(NAMES)))
    parser.add_argument('--output', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    fixture = ROOT / 'tests/fixtures/mir-regions-oracle.ddr'
    support = ROOT / 'tests/fixtures/mir_region_qa_support.ddr'
    declaration = (ROOT / 'selfhost/mir.ddr').read_text().split('pub struct Function {', 1)[1].split('\n}', 1)[0]
    metadata = support.read_text().split('pub fn metadata(', 1)[1]
    fields = set(re.findall(r'\b([a-z][a-z0-9_]*)\s*:', re.sub(r'//[^\n]*', '', declaration)))
    observed = set(re.findall(r'\bf\.([a-z][a-z0-9_]*)', metadata))
    assert fields == observed, (fields - observed, observed - fields)
    report = {'host': host, 'fields': len(fields), 'hashes': {}, 'images': []}
    for key, path in dict(compiler=compiler, fixture=fixture, support=support,
                          ssa=ROOT/'selfhost/mir_ssa.ddr', frame_abi=ROOT/'selfhost/mir_call_abi.ddr').items():
        report['hashes'][key] = digest(path)
    with tempfile.TemporaryDirectory(prefix='dunder-regional-qa-') as temp:
        work = args.output.resolve() if args.output else Path(temp)
        if args.output:
            work.mkdir(parents=True, exist_ok=True)
            assert not any(work.iterdir()), 'output must be empty'

        def call(command, status=0):
            result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True, text=True, timeout=60)
            assert result.returncode == status, (command, result.returncode, result.stdout[-5000:], result.stderr[-5000:])
            return result.stdout

        emitter = work / 'emitter'
        call([compiler, 'build', fixture, '-o', emitter])
        report['hashes']['emitter'] = digest(emitter)

        def one(case, value, control, target, mode, frame=0, limit=LIMIT, terminal=1, label=''):
            image = work/f'{case}-{value}-{control}-{target}-{mode}-{frame}-{terminal}-{limit}{label}'
            text = call([emitter, image, target, case, mode, value, control, expected(case,value,control), limit, frame, terminal])
            helper_text = text.split('HELPERS\n',1)[1].split('ENDHELPERS\n',1)[0]
            helpers = list(map(int,helper_text.splitlines()))
            export, capture_text = text.split('CAPTURES\n',1)
            before, after, bm, am, counters = qa.parse_export(export)
            counters['captures'] = int(capture_text.strip())
            assert 0 <= counters['units'] <= limit, counters
            for index, op in enumerate(before['operations']):
                if op['kind'] in (8,14) and op['mode'] != 2:
                    assert after['operations'][index] == op, ('pinned store/call',case,index)
            pre = interpret(before,helpers,case,value,control)
            post = interpret(after,helpers,case,value,control)
            assert pre.get('result') == post.get('result') and pre.get('trap') == post.get('trap')
            traffic_before, traffic_after = qa.traffic(before), qa.traffic(after)
            if mode == 'off' or not counters['changed']:
                assert bm == am and before == after, (NAMES[case],frame,counters,'noncommit mutated Function')
            if mode == 'off': assert counters['status'] == counters['units'] == 0
            elif limit < LIMIT:
                assert not counters['changed'] and counters['status'] == -3, counters
            elif case == 13 and not terminal:
                assert not counters['changed'] and counters['status'] == -2, counters
            elif case in NEGATIVE or case == 10:
                assert not counters['changed'] and counters['captures'] == 0, counters
            elif frame:
                assert counters['changed'] and counters['status'] == 2 and counters['captures'] > 0, (NAMES[case],counters)
                assert traffic_after['loads'] < traffic_before['loads'], (NAMES[case],traffic_before,traffic_after,counters)
            else: assert counters['captures'] == 0, counters
            trap = case == 13 and control == 0
            ran = False
            if target == host: call([image],102 if trap else 0); ran = True
            elif target == 3: call(['node',ROOT/'tools/runtime-contract-wasm.cjs',image],125 if trap else 0); ran = True
            if args.output: image.with_suffix('.mir.txt').write_text(text)
            report['images'].append(dict(case=case,name=NAMES[case],input=value,control=control,target=target,
                mode=mode,frame=frame,terminal=terminal,limit=limit,executed=ran,sha256=digest(image),counters=counters,
                before_traffic=traffic_before,after_traffic=traffic_after,before=pre,after=post))
            return counters

        samples = [x for x in corpus() if args.case is None or x[0] == args.case]
        for index,(case,value,control) in enumerate(samples):
            for target in range(4):
                one(case,value,control,target,'off')
                one(case,value,control,target,'on')
                one(case,value,control,target,'on',frame=1)
                if case == 13: one(case,value,control,target,'on',frame=1,terminal=0)
            print(f'PASS {index+1}/{len(samples)} {NAMES[case]} input{value} control{control}',flush=True)
        for case,value,control in ((1,41,0x800000FF),(4,41,1),(7,41,4),(14,41,17),(12,41,17)):
            if args.case is not None and case != args.case: continue
            for target in (host,3):
                full = one(case,value,control,target,'on',frame=1,label='-budget-base')
                for limit in sorted({0,1,max(1,full['units']//2),max(1,full['units']-1)}):
                    one(case,value,control,target,'on',frame=1,limit=limit,label='-budget')
    report.update(images_built=len(report['images']),executions=sum(x['executed'] for x in report['images']),
                  independent_pairs=sum('gap' not in x['before'] for x in report['images']),
                  coverage_limits=['large retained-helper scratch is checked by actual native/Wasm only',
                                   'Linux ARM/x86 images build only on this macOS host'])
    if args.report:
        args.report.parent.mkdir(parents=True,exist_ok=True)
        args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
    print(f'{report["images_built"]} images; {report["executions"]} native/Wasm executions; {report["independent_pairs"]} independent helper-semantic pairs; no timings')


if __name__ == '__main__': main()
