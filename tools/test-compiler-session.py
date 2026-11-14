#!/usr/bin/env python3
"""Resumable frontend contracts; optional frozen-compiler image/diagnostic check.

No timings. The old-syntax consumer drives two independent Function/Object
roots, reverses backend completion and accepts only source ordinals in order.
It compares all target images/imports/bindings, cache reuse and failure paths.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


def run(command, *, success=True):
    result = subprocess.run([str(x) for x in command], cwd=ROOT,
                            capture_output=True, text=True)
    if (result.returncode == 0) != success:
        raise AssertionError(f'{command}\n{result.stdout}{result.stderr}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve()
    baseline = args.baseline.resolve() if args.baseline else None
    output = ROOT / 'build' / 'session-qa'
    output.mkdir(parents=True, exist_ok=True)
    fixture = ROOT / 'tests' / 'fixtures' / 'compiler-session.ddr'
    rows = []
    with tempfile.TemporaryDirectory(prefix='contracts-', dir=output) as temporary:
        work = Path(temporary)
        consumer = work / 'consumer'
        run([compiler, 'build', fixture, '-o', consumer])
        result = run([consumer])
        assert result.stdout == 'session-passed\n', result.stdout
        if baseline:
            # Real frozen runtime consumers exercise the shared serial adapter,
            # not smaller replacement loops constructed for this refactor.
            for source in sorted((ROOT / 'benchmarks' / 'runtime').glob('*.ddr')):
                for target in TARGETS:
                    old = work / (source.stem + '-' + target + '-old')
                    new = work / (source.stem + '-' + target + '-new')
                    run([baseline, 'build', source, '--target', target, '-o', old])
                    run([compiler, 'build', source, '--target', target, '-o', new])
                    image = old.read_bytes()
                    assert image == new.read_bytes(), (source, target)
                    rows.append({'source': str(source.relative_to(ROOT)), 'target': target,
                                 'sha256': hashlib.sha256(image).hexdigest()})
            portable = (
                'fn first(n:i64)->i64{n*3} fn last(a:[]i64)->[]i64{var b:=a;b[0]=b[0]+1;b} fn main(){a:[]i64=[41];print(last(a)[0]);print(first(14))}',
                'fn main(){value:f64=2.5;print(value.str())}',
            )
            for index, source in enumerate(portable):
                path = work / f'portable-{index}.ddr'
                path.write_text(source)
                for target in TARGETS:
                    old, new = work / f'old-{index}-{target}', work / f'new-{index}-{target}'
                    run([baseline, 'build', path, '--target', target, '-o', old])
                    run([compiler, 'build', path, '--target', target, '-o', new])
                    image = old.read_bytes()
                    assert image == new.read_bytes(), (index, target)
                    rows.append({'source': source, 'target': target,
                                 'sha256': hashlib.sha256(image).hexdigest()})
            # Reachable/dead-body and expression errors retain diagnostics.
            invalid = (
                'fn bad()->i64{false} fn main(){print(bad())}',
                'fn dead()->i64{false} fn main(){print(42)}',
                'fn main(){var a:[]i64=[1];print(a[false])}',
                'fn main(){for i in false..3{print(i)}}',
            )
            for index, source in enumerate(invalid):
                path = work / f'invalid-{index}.ddr'
                path.write_text(source)
                for action in ('build', 'check'):
                    old = run([baseline, action, path], success=False)
                    new = run([compiler, action, path], success=False)
                    assert (old.returncode, old.stdout, old.stderr) == (new.returncode, new.stdout, new.stderr)
    report = {'consumer': 'passed', 'identity_images': rows,
              'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest()}
    if baseline:
        report['baseline_sha256'] = hashlib.sha256(baseline.read_bytes()).hexdigest()
    (output / 'contracts.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Session contracts passed; {len(rows)} frozen image comparisons')


if __name__ == '__main__':
    main()
