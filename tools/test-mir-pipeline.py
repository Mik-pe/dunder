#!/usr/bin/env python3
"""Finite shared/reference MIR stage differential; no latency measurements.

Checks exact semantic rows, pass statuses/counters, all four image targets and
native-host/Wasm results. Also executes a real isolated source-task worker.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('pipeline_oracle', ROOT / 'tools/test-mir.py')
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)
FIELDS = ('error frame_state value_state operations opaque optimized promoted removed_loads removed_stores '
          'private_memory_ops analysis_units budget_refusals values_optimized value_rewrites value_deleted '
          'value_immediates value_analysis_units value_budget_refusals').split()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def call(command, cwd, status=0):
    result = subprocess.run(list(map(str, command)), cwd=cwd, text=True, capture_output=True, timeout=120)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def expected(case, value):
    if case == 0: return oracle.signed(value + 5)
    if case == 4: return oracle.signed(value + 3)
    if case == 8: return oracle.signed(value + 99)
    if case == 9: return oracle.signed(value + 17)
    if case == 10: return oracle.signed(value + 1)
    if case == 13: return oracle.signed(value)
    raise AssertionError(case)


def interpret(model, value, condition, case, traps):
    machine = oracle.Interpreter(model)
    # A specified ABI input, including caller-observable preserved GP values
    # and valid stack bytes. Promotion may save/read these bytes before source
    # stores; the same complete initial state is used for original and result.
    for reg in range(29):
        machine.variables[reg] = 0
    machine.variables.update({0: condition & oracle.MASK64, 4: value & oracle.MASK64})
    for address in range((1 << 20) - 4096, (1 << 20) + 256):
        machine.memory[address] = 0
    try:
        result = oracle.signed(machine.run())
        assert not traps
        return {'result': result, 'steps': machine.steps}
    except oracle.CoverageGap as gap:
        text = str(gap)
        if case == 8:
            assert 'kind=14' in text, text
        else:
            assert traps and 'external branch' in text, text
        return {'coverage_gap': text, 'steps': machine.steps}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    report = {'compiler_sha256': digest(compiler), 'pipeline_sha256': digest(ROOT / 'selfhost/mir_pipeline.ddr'),
              'reference_sha256': digest(ROOT / 'tests/fixtures/mir-pipeline-reference.ddr'),
              'fixture_sha256': digest(ROOT / 'tests/fixtures/mir-pipeline-emitter.ddr'), 'images': []}
    pairs = executions = proofs = gaps = 0
    with tempfile.TemporaryDirectory(prefix='dunder-mir-pipeline-') as directory:
        work = Path(directory); emitter = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/mir-pipeline-emitter.ddr', '-o', emitter], work)
        report['emitter_sha256'] = digest(emitter)
        corpus = [(case, value, condition) for case in (0, 4, 8, 9, 10, 13)
                  for value, condition in ((41, 0), (-17, 1))]
        corpus.append((10, 9223372036854775807, 0))
        for index, (case, value, condition) in enumerate(corpus):
            result = expected(case, value); traps = case == 10 and value == 9223372036854775807
            for policy in range(4):  # Off, conservative raw, source+terminals, exhausted budgets.
                for target in range(4):
                    reference = None
                    for mode in ('reference', 'pipeline'):
                        image = work / f'{index}-{policy}-{target}-{mode}'
                        output = call([emitter, image, target, case, mode, value, condition, result, policy], work).stdout
                        original, rewritten = output.split('ORIGINAL\n', 1)[1].split('RESULT\n', 1)
                        rewritten, raw_counters = rewritten.split('COUNTERS\n', 1)
                        before = oracle.decode_model(original); after = oracle.decode_model(rewritten)
                        counters = dict(zip(FIELDS, map(int, raw_counters.split()), strict=True))
                        assert counters['error'] == 0 and counters['operations'] == len(after['order'])
                        assert counters['opaque'] == sum(op['kind'] == 0 for op in after['operations'])
                        if policy == 0:
                            assert counters['frame_state'] == counters['value_state'] == 0
                        if policy == 3:
                            # Frame's no-place refusal precedes any charged
                            # analysis, exactly as in the existing pipeline.
                            assert counters['frame_state'] in (-2, -3)
                            assert counters['budget_refusals'] == int(counters['frame_state'] == -3)
                            assert counters['value_budget_refusals'] == 1
                            assert counters['optimized'] == counters['values_optimized'] == 0
                        pre = interpret(before, value, condition, case, traps)
                        post = interpret(after, value, condition, case, traps)
                        assert ('coverage_gap' in pre) == ('coverage_gap' in post)
                        if 'coverage_gap' in pre:
                            gaps += 1
                        else:
                            assert pre['result'] == post['result'] == result, (case, policy, mode, value, pre, post)
                            proofs += 1
                        image_sha = digest(image)
                        evidence = (original, rewritten, counters, image_sha)
                        if reference is None:
                            reference = evidence
                        else:
                            assert reference == evidence, ('shared/reference mismatch', case, policy, target, value)
                            pairs += 1
                        ran = False
                        if target == host:
                            call([image], work, 101 if traps else 0); executions += 1; ran = True
                        if target == 3:
                            call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work, 125 if traps else 0)
                            executions += 1; ran = True
                        report['images'].append(dict(case=case, input=value, condition=condition, policy=policy,
                                                     target=target, mode=mode, sha256=image_sha, executed=ran,
                                                     counters=counters, before=pre, after=post))
            print(f'PASS {index + 1}/{len(corpus)} case{case} input{value}', flush=True)
        if host == 0:
            worker = work / 'worker'
            call([compiler, 'build', ROOT / 'tests/fixtures/mir-pipeline-worker.ddr', '-o', worker], work)
            assert call([worker], work).stdout == '42\n'
            report['worker'] = {'executed': True, 'sha256': digest(worker), 'result': 42}
        else:
            report['worker'] = {'executed': False, 'coverage_gap': 'task provider requires ARM64/macOS'}
    report.update(exact_reference_pairs=pairs, images_built=len(report['images']), executions=executions,
                  oracle_pairs=proofs, oracle_gap_pairs=gaps)
    report['manifest_sha256'] = hashlib.sha256(json.dumps(report['images'], sort_keys=True).encode()).hexdigest()
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'{pairs} exact reference pairs; {len(report["images"])} images; {executions} host/Wasm executions; '
          f'{proofs} oracle pairs; {gaps} explicit call/trap gaps; worker={report["worker"]["executed"]}')
    print('manifest_sha256=' + report['manifest_sha256'])


if __name__ == '__main__':
    main()
