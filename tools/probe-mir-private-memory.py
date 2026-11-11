#!/usr/bin/env python3
"""Probe unchanged text.ddr with inline ON after trusted producer wiring.

Two fresh CLI builds and one small native result check per compiler; no timing
series. This is aggregate whole-source MIR evidence, not a per-loop dump.
Standalone test-mir-memory.py supplies exact per-loop alias/sync contracts.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'benchmarks/runtime/text.ddr'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def call(command, work):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=60)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    compilers = {'baseline': args.baseline.resolve(strict=True),
                 'candidate': args.compiler.resolve(strict=True)}
    host = 'arm64-macos' if platform.system() == 'Darwin' else (
        'arm64-linux' if platform.machine() in ('arm64','aarch64') else 'x86_64-linux')
    source_hash = digest(SOURCE)
    sample = 'Hej  åäö 😀\n世界\tabc'
    words, inside = 0, False
    for scalar in sample:
        if scalar in ' \n\t': inside = False
        elif not inside: words, inside = words + 1, True
    iterations = 3
    expected = str(iterations * (words * 1000000 + sum(map(ord, sample)))) + '\n'
    evidence = {'scope': 'whole_source_aggregate_mir_counters', 'per_loop_dump': False,
                'source': str(SOURCE), 'source_sha256': source_hash,
                'target': host, 'inline': 'on', 'incremental': False,
                'expected_policy': 'mir-memory:source-objects-v1', 'compilers': {}}
    with tempfile.TemporaryDirectory(prefix='dunder-private-source-probe-') as directory:
        work = Path(directory)
        for name, compiler in compilers.items():
            image = work / name
            command = [compiler, 'build', SOURCE, '--target', host, '--inline-opt=on',
                       '--mir=on', '--mir-opt=on', '--timings=json', '-o', image]
            built = call(command, work)
            reports = [json.loads(line) for line in built.stderr.splitlines() if line.startswith('{')]
            assert len(reports) == 1 and reports[0]['kind'] == 'timings', built.stderr
            report = reports[0]; backend = report['mir']
            assert report['inlining']['enabled'] is True
            assert backend['enabled'] is True and backend['optimization_enabled'] is True
            assert report['functions']['reused'] == 0 and backend['functions'] > 0
            result = call([image, iterations, sample], work)
            assert result.stdout == expected, (name, result.stdout, expected)
            evidence['compilers'][name] = {'path': str(compiler), 'sha256': digest(compiler),
                'image_sha256': digest(image), 'image_bytes': image.stat().st_size,
                'mir': backend, 'inlining': report['inlining'], 'result': result.stdout.strip()}
    assert digest(SOURCE) == source_hash, 'probe source changed'
    before = evidence['compilers']['baseline']['mir']
    after = evidence['compilers']['candidate']['mir']
    assert before.get('private_memory_ops', 0) == 0, 'baseline must use the conservative raw policy'
    assert after.get('private_memory_ops', 0) > 0, 'candidate producer has not enabled/reported private storage'
    assert evidence['compilers']['candidate']['image_sha256'] != evidence['compilers']['baseline']['image_sha256'], 'reported promotion did not change the program image'
    loads = after['removed_loads'] - before['removed_loads']
    stores = after['removed_stores'] - before['removed_stores']
    assert loads > 0 and stores > 0, ('no additional frame promotion in unchanged inline-ON text source', before, after)
    evidence['delta_removed_loads'] = loads; evidence['delta_removed_stores'] = stores
    output = args.output.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    print(f'PASS unchanged text.ddr inline ON: {loads} additional static frame loads and {stores} stores removed; '
          f'{after["private_memory_ops"]} private indirect positions; native result matched')
    print(output)


if __name__ == '__main__':
    main()
