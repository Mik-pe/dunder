#!/usr/bin/env python3
"""Source-level SSA acceptance: results, workers, diagnostics, cache and receipts.

Build all four targets and execute the native host and Wasm. This is a
functional test, without latency assertions or Linux execution on macOS.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
import wasm_host


FUNCTION = '''fn calculate(n:i32)->i64 {
    var a:i64=1;var b:i64=2;var c:i64=3;var d:i64=4
    var e:i64=5;var f:i64=6;var g:i64=7;var i:i32=0
    while i<n {
        a=a.wrapping_add(i64(i));b=b.wrapping_add(a);c=c.wrapping_add(b)
        d=d.wrapping_add(c);e=e.wrapping_add(d);f=f.wrapping_add(e);g=g.wrapping_add(f)
        i=i.wrapping_add(1)
    }
    a.bit_xor(b).bit_xor(c).bit_xor(d).bit_xor(e).bit_xor(f).bit_xor(g)
}
'''
COUNTERS = tuple('ssa_' + name for name in (
    'optimized', 'cells', 'values', 'parameters', 'registers', 'captures', 'stack_groups', 'removed_loads',
    'removed_stores', 'removed_pushes', 'removed_pops', 'copies',
    'analysis_units', 'budget_refusals'))
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
# Isolate SSA from the local scalar frontend, inlining and value cleanup;
# the preceding whole-function frame pass remains enabled.
FLAGS = ('--scalar-opt=off', '--inline-opt=off', '--mir-values=off')


def reference(n):
    values = list(range(1, 8))
    mask = (1 << 64) - 1
    for i in range(n):
        values[0] = (values[0] + i) & mask
        for j in range(1, len(values)):
            values[j] = (values[j] + values[j - 1]) & mask
    answer = 0
    for value in values:
        answer ^= value
    return answer if answer < (1 << 63) else answer - (1 << 64)


def call(command, cwd, status=0):
    result = subprocess.run(list(map(str, command)), cwd=cwd, capture_output=True,
                            text=True, timeout=120)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def execute(image, target, host, expected, work, wasm_worker):
    if target == host:
        result = call([image], work)
    elif target == 'wasm32':
        result = wasm_worker.run(image)
        assert result.returncode == 0, result
    else:
        return False
    assert result.stdout == expected and result.stderr == '', result
    return True


def counters(report, active, productive=False):
    data = report['mir']
    assert data['ssa_enabled'] is active, data
    assert all(type(data[key]) is int and data[key] >= 0 for key in COUNTERS), data
    if not active:
        assert all(data[key] == 0 for key in COUNTERS), data
    if productive:
        for key in ('ssa_optimized', 'ssa_cells', 'ssa_values', 'ssa_parameters',
                    'ssa_registers', 'ssa_removed_loads', 'ssa_analysis_units'):
            assert data[key] > 0, (key, data)
    return {key: data[key] for key in COUNTERS}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = ('arm64-macos' if platform.system() == 'Darwin' else
            'arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
    workers = (1, 2, 4) if host == 'arm64-macos' else (1,)
    inputs = (0, 1, 6, 31)
    expected = ''.join(str(reference(n)) + '\n' for n in inputs)
    source = FUNCTION + 'fn main(){' + ';'.join(f'print(calculate({n}))' for n in inputs) + '}\n'
    images = executions = 0
    with tempfile.TemporaryDirectory(prefix='dunder-ssa-source-') as directory, wasm_host.WasmWorker() as wasm_worker:
        work = Path(directory)
        path = work/'main.ddr'; path.write_text(source)
        variants = {}
        for target in TARGETS:
            for mode in ('off', 'on'):
                same = None
                for jobs in workers:
                    image = work/f'{target}-{mode}-{jobs}'
                    result = call([compiler, 'build', path, '--target', target, '--jobs=' + str(jobs),
                                   '--mir-ssa=' + mode, *FLAGS, '--timings=json', '-o', image], work)
                    report = json.loads(result.stderr)
                    counts = counters(report, mode == 'on', mode == 'on')
                    observed = (hashlib.sha256(image.read_bytes()).hexdigest(), counts)
                    assert same is None or same == observed, (target, mode, jobs, same, observed)
                    same = observed
                    images += 1
                    executions += execute(image, target, host, expected, work, wasm_worker)
                variants[target, mode] = same[0]
            assert variants[target, 'off'] != variants[target, 'on'], target

        # Exercise the real producer annotations and its separate storage
        # permission, including the same combined allocator in every worker.
        expression = work/'expression.ddr'
        expression.write_text('fn calculate(a:i64,b:i64,c:i64)->i64{(a+b)*(b+c)}\n'
                              'fn main(){print(calculate(2,3,5))}\n')
        for target in TARGETS:
            modes = []
            for mode in ('off', 'on'):
                same = None
                for jobs in workers:
                    image = work/f'expression-{target}-{mode}-{jobs}'
                    result = call([compiler, 'build', expression, '--target', target,
                                   '--jobs=' + str(jobs), '--mir-ssa=' + mode, *FLAGS,
                                   '--timings=json', '-o', image], work)
                    counts = counters(json.loads(result.stderr), mode == 'on')
                    if mode == 'on':
                        for key in ('ssa_stack_groups', 'ssa_removed_pushes', 'ssa_removed_pops'):
                            assert counts[key] > 0, (key, counts)
                    observed = (hashlib.sha256(image.read_bytes()).hexdigest(), counts)
                    assert same is None or same == observed, (target, mode, jobs, same, observed)
                    same = observed
                    images += 1
                    executions += execute(image, target, host, '40\n', work, wasm_worker)
                modes.append(same[0])
            assert modes[0] != modes[1], target

        checks = []
        for mode in ('off', 'on'):
            result = call([compiler, 'check', path, '--agent', '--mir-ssa=' + mode,
                           *FLAGS, '--timings=json'], work)
            counters(json.loads(result.stderr), False)
            checks.append(result.stdout)
        assert checks[0] == checks[1]
        for disabled in ('--mir=off', '--mir-opt=off'):
            result = call([compiler, 'build', path, '--mir-ssa=on', disabled,
                           *FLAGS, '--timings=json', '-o', work/'disabled'], work)
            counters(json.loads(result.stderr), False)

        for mode, reused in (('off', 0), ('off', 2), ('on', 0), ('on', 2), ('off', 2)):
            image = work/'cached'
            result = call([compiler, 'build', path, '--mir-ssa=' + mode, *FLAGS,
                           '--incremental', '--timings=json', '-o', image], work)
            report = json.loads(result.stderr)
            assert report['functions'] == {'reused': reused, 'emitted': 2-reused}, report
            counts = counters(report, mode == 'on', mode == 'on' and reused == 0)
            if reused == 2:
                assert all(value == 0 for value in counts.values()), report
            assert hashlib.sha256(image.read_bytes()).hexdigest() == variants[host, mode]
            executions += execute(image, host, host, expected, work, wasm_worker)

        tests = work/'cases.ddr'
        tests.write_text(FUNCTION + '@test fn result(){calculate(6)==' + str(reference(6)) + '}\n')
        receipts = {}
        output = work/'results'
        for mode, text in (('off', '1 passed'), ('off', '1 unaffected'),
                           ('on', '1 passed'), ('on', '1 unaffected'), ('off', '1 passed')):
            result = call([compiler, 'test', tests, '--mir-ssa=' + mode, *FLAGS,
                           '--affected', '-o', output], work)
            assert text in result.stdout, result
            saved = list((output/'.affected').glob('*.pass'))
            assert len(saved) == 1
            receipt = saved[0].read_bytes()
            assert mode not in receipts or receipt == receipts[mode]
            receipts[mode] = receipt
        assert receipts['off'] != receipts['on']

        bad = work/'bad.ddr'; bad.write_text('fn main(){unknown()}\n')
        errors = []
        original = (work/'cached').read_bytes()
        for mode in ('off', 'on'):
            errors.append(call([compiler, 'check', bad, '--agent', '--mir-ssa=' + mode], work, 1).stdout)
            call([compiler, 'build', bad, '--mir-ssa=' + mode, '-o', work/'cached'], work, 1)
            assert (work/'cached').read_bytes() == original
        assert errors[0] == errors[1]
        call([compiler, 'check', path, '--mir-ssa=unknown'], work, 2)
    print(f'PASS SSA source: {images} four-target images, {executions} native/Wasm executions; '
          'productive placement, exact workers, diagnostics, cache and receipts; no latency claims')


if __name__ == '__main__':
    main()
