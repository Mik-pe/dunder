#!/usr/bin/env python3
"""Finite off/on value-lattice tests with an independent semantic MIR oracle.

Build all four actual image targets, execute only the native host and Wasm,
and report interpreter gaps (calls/trap exits) separately from proven results.
No latency series and no production activation are performed.
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
spec = importlib.util.spec_from_file_location('semantic_mir_oracle', ROOT / 'tools/test-mir.py')
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def call(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, text=True, capture_output=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def expected(case, value, condition):
    if case == 0: return oracle.signed(value + 5)
    if case in (1, 3): return oracle.signed(value + 11)
    if case == 2: return oracle.signed(value + (12 if condition == 0 else 11))
    if case == 4: return oracle.signed(value + 3)
    if case == 5: return value & 0xFFFFFFFF
    if case == 6: return (value + condition) & 0xFFFFFFFF
    if case in (7, 12): return oracle.signed(value + 1)
    if case == 8: return oracle.signed(value + 99)
    if case == 9: return oracle.signed(value + 17)
    if case == 10: return oracle.signed(value + 1)
    if case == 11: return 0
    if case == 13: return oracle.signed(value)
    raise AssertionError(case)


def interpret(model, value, condition, case, traps):
    machine = oracle.Interpreter(model)
    machine.variables.update({0: condition & oracle.MASK64, 4: value & oracle.MASK64})
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
    parser.add_argument('--compare-report', type=Path,
                        help='require the same SHA256 for every program image in a prior successful matrix')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    corpus = []
    for case in range(14):
        inputs = [(41, 0), (-17, 1)]
        if case == 4: inputs += [(9223372036854775806, 0)]
        if case in (5, 6): inputs += [(oracle.signed(0xFEDCBA98FFFFFFFF), 1)]
        if case == 10: inputs += [(9223372036854775807, 0)]
        if case == 11: inputs = [(41, 0), (-17, 0)]
        corpus.extend((case, value, condition) for value, condition in inputs)
    report = {'compiler_sha256': digest(compiler), 'host_target': host,
              'value_module_sha256': digest(ROOT / 'selfhost/mir_value_prop.ddr'),
              'fixture_sha256': digest(ROOT / 'tests/fixtures/mir-value-emitter.ddr'), 'images': []}
    gaps = 0; proofs = 0; executed = 0
    with tempfile.TemporaryDirectory(prefix='dunder-mir-values-') as directory:
        work = Path(directory); emitter = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/mir-value-emitter.ddr', '-o', emitter], work)
        report['emitter_sha256'] = digest(emitter)
        for index, (case, value, condition) in enumerate(corpus):
            result = expected(case, value, condition)
            traps = case == 10 and value == 9223372036854775807
            for target in range(4):
                for mode in ('off', 'on'):
                    image = work / f'value-{index}-{target}-{mode}'
                    output = call([emitter, image, target, case, mode, value, condition, result], work).stdout
                    original, rewritten = output.split('ORIGINAL\n', 1)[1].split('RESULT\n', 1)
                    rewritten, counters = rewritten.split('COUNTERS\n', 1)
                    before = oracle.decode_model(original); after = oracle.decode_model(rewritten)
                    pre = interpret(before, value, condition, case, traps)
                    post = interpret(after, value, condition, case, traps)
                    assert ('coverage_gap' in pre) == ('coverage_gap' in post)
                    if 'coverage_gap' in pre:
                        gaps += 1
                    else:
                        assert pre['result'] == post['result'] == result, (case, mode, value, pre, post, result)
                        proofs += 1
                    rewrites, deleted, immediates, units = map(int, counters.split())
                    if mode == 'off': assert (rewrites, deleted, immediates, units) == (0, 0, 0, 0)
                    if mode == 'on' and case in (0, 1, 3, 6, 7, 12, 13):
                        assert rewrites + deleted > 0, (case, rewrites, deleted)
                    if case in (4, 10) and mode == 'on':
                        assert rewrites == deleted == immediates == 0
                    if case == 7 and mode == 'on':
                        assert all(op['mode'] == 2 for op in after['operations'] if op['kind'] in (1, 3)), after
                    ran = False
                    if target == host:
                        call([image], work, 101 if traps else 0); executed += 1; ran = True
                    if target == 3:
                        call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], work, 125 if traps else 0)
                        executed += 1; ran = True
                    report['images'].append(dict(case=case, input=value, condition=condition, target=target, mode=mode,
                                                 sha256=digest(image), executed=ran, trap=traps, before=pre, after=post,
                                                 rewrites=rewrites, deleted=deleted, immediates=immediates, units=units))
            print(f'PASS {index + 1}/{len(corpus)} case{case} input{value} condition{condition}', flush=True)
    report.update(images_built=len(report['images']), executions=executed, oracle_pairs=proofs, oracle_gap_pairs=gaps)
    report['manifest_sha256'] = hashlib.sha256(json.dumps(report['images'], sort_keys=True).encode()).hexdigest()
    if args.compare_report:
        previous = json.loads(args.compare_report.read_text())
        key = lambda row: (row['case'], row['input'], row['condition'], row['target'], row['mode'])
        reference = {key(row): row['sha256'] for row in previous['images']}
        assert len(reference) == len(report['images'])
        for row in report['images']:
            assert reference[key(row)] == row['sha256'], ('program image changed', key(row), reference[key(row)], row['sha256'])
        report['reference_manifest_sha256'] = previous['manifest_sha256']
        report['reference_program_sha256_matches'] = len(report['images'])
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'{len(corpus)} programs/inputs; {len(report["images"])} images; {executed} host/Wasm executions; '
          f'{proofs} independent oracle pairs; {gaps} explicitly reported call/trap gaps')
    print('manifest_sha256=' + report['manifest_sha256'])
    if args.compare_report:
        print(f'{len(report["images"])} program image SHA256s match the previous implementation')


if __name__ == '__main__':
    main()
