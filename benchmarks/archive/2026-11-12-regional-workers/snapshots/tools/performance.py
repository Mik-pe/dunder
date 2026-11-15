#!/usr/bin/env python3
"""Compare native selfhost compilers using checked outputs and fresh processes."""
import argparse
import csv
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH': '/no-external-tools'}
TEXT = ('Dunder läser UTF-8: åäö, Ελληνικά och 🌍.\n'
        'Säkra värden överlever sina anrop; loopar räknar byte och runor.\t'
        'Native kod utan externa byggverktyg. ') * 4


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args, cwd=ROOT, env=ENV):
    start = time.perf_counter_ns()
    result = subprocess.run([str(a) for a in args], cwd=cwd, env=env,
                            capture_output=True, timeout=120)
    elapsed = (time.perf_counter_ns() - start) / 1_000_000
    if result.returncode:
        raise RuntimeError(f'{args}: exit {result.returncode}\n'
                           + (result.stdout + result.stderr).decode(errors='replace'))
    return elapsed, result.stdout


def checked_run(binary, args, expected):
    elapsed, output = command([binary, *args])
    if output != expected:
        raise RuntimeError(f'{binary}: expected {expected!r}, got {output!r}')
    return elapsed


def runtime_cases():
    seed = 1729
    value, numeric = seed, 0
    for _ in range(1_000_000):
        value = (value * 1664525 + 1013904223) % 2147483647
        numeric += value
    values = [(i * 17 + seed) % 1009 for i in range(1024)]
    scan = 0
    for i in range(3000):
        values[i % 1024] = i
        scan += sum(values)
    words, inside = 0, False
    for scalar in TEXT:
        if scalar in ' \n\t':
            inside = False
        elif not inside:
            words, inside = words + 1, True
    text = (words * 1_000_000 + sum(map(ord, TEXT))) * 2000
    sort = 0
    for repeat in range(120):
        values = sorted(((255-i) * 17 + seed + repeat) % 1009 for i in range(256))
        sort += sum(value * (i + 1) for i, value in enumerate(values))
    return [('numeric', ['1000000', str(seed)], numeric),
            ('scan', ['3000', str(seed)], scan),
            ('text', ['2000', TEXT], text),
            ('sort', ['120', str(seed)], sort)]


def corpus(work):
    root = work / 'corpus'
    root.mkdir()
    main = ''.join(f'import m{i}\n' for i in range(20))
    for module in range(20):
        source = ''.join(f'pub fn item{f}(x: i64) -> i64 {{\n    var value := x\n'
                         + '    value = value + 1\n' * 96
                         + '    return value\n}\n' for f in range(50))
        (root / f'm{module}.ddr').write_text(source)
    main += 'fn main() {\n    var total: i64 = 0\n'
    for module in range(20):
        for f in range(50):
            main += f'    total = total + m{module}.item{f}({module * 50 + f})\n'
    main += '    print(total)\n}\n'
    (root / 'main.ddr').write_text(main)
    sources = [path.read_bytes() for path in sorted(root.glob('*.ddr'))]
    # This generated corpus has no strings or comments; this pattern exactly
    # counts its lexical tokens, including the two-character operators.
    token = rb'[A-Za-z_0-9]+|:=|->|==|!=|<=|>=|&&|\|\||\.\.|[^\s]'
    info = {'lines': sum(len(s.splitlines()) for s in sources),
            'source_bytes': sum(map(len, sources)), 'modules': 21, 'functions': 1001,
            'tokens': sum(len(re.findall(token, s)) for s in sources),
            'sha256': hashlib.sha256(b''.join(sources)).hexdigest()}
    return root / 'main.ddr', info


def source_hashes():
    paths = sorted(set(ROOT.glob('selfhost/*.ddr')) | set(ROOT.glob('stdlib/*.ddr'))
                   | set(ROOT.glob('runtime/*.ddr')) | set(ROOT.glob('benchmarks/runtime/*.ddr'))
                   | set(ROOT.glob('projects/tabell/**/*.ddr'))
                   | {ROOT / 'benchmarks/runtime/reference.c',
                      ROOT / 'projects/tabell/expenses.csv', ROOT / 'projects/tabell/expected.csv'})
    return {str(p.relative_to(ROOT)): digest(p) for p in paths}


def order(variants, round_number):
    if len(variants) == 2:
        return variants if round_number % 2 == 0 else list(reversed(variants))
    return list(list(itertools.permutations(variants))[round_number % 6])


def save_csv(path, rows):
    if rows:
        with path.open('w', newline='') as output:
            writer = csv.DictWriter(output, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('baseline', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('report', type=Path)
    parser.add_argument('--rounds', type=int, default=30)
    parser.add_argument('--suite', choices=['all', 'runtime', 'compiler'], default='all')
    parser.add_argument('--include-check', action='store_true',
                        help='also measure check on generated and selfhost sources')
    parser.add_argument('--c-compiler', type=Path, help='also measure C with -O3 -ftrapv')
    parser.add_argument('--baseline-flag', action='append', default=[],
                        help='extra compiler flag, e.g. --baseline-flag=--scalar-opt=off')
    parser.add_argument('--candidate-flag', action='append', default=[],
                        help='extra compiler flag for the candidate variant')
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error('--rounds must be positive')
    if args.include_check and args.suite == 'runtime':
        parser.error('--include-check requires the compiler or all suite')
    compilers = {'baseline': args.baseline.resolve(strict=True),
                 'candidate': args.candidate.resolve(strict=True)}
    compiler_flags = {'baseline': args.baseline_flag, 'candidate': args.candidate_flag}
    report = args.report.resolve()
    report.mkdir(parents=True, exist_ok=True)
    if any(report.iterdir()):
        parser.error('report directory must be empty to preserve existing measurements')
    before = source_hashes()
    metadata = {'host': platform.uname()._asdict(), 'rounds': args.rounds,
                'suite': args.suite, 'include_check': args.include_check,
                'checkout': command(['git', 'rev-parse', 'HEAD'], env=None)[1].decode().strip(),
                'working_tree': command(['git', 'status', '--short'], env=None)[1].decode(),
                'tool_sha256': digest(Path(__file__)), 'sources_sha256': before,
                'compiler_sha256': {v: digest(p) for v, p in compilers.items()},
                'compiler_paths': {v: str(p) for v, p in compilers.items()},
                'compiler_flags': compiler_flags,
                'clock': 'wall clock including process startup; perf_counter_ns',
                'cache': 'incremental disabled; warm OS filesystem cache',
                'order': 'two variants alternate each round; three rotate all six permutations',
                'warmup': 'one unrecorded execution per runtime binary; no compile warmup',
                'environment': ENV, 'profile': 'default safety checks enabled',
                'target': {'Darwin': 'arm64-macos', 'Linux': 'arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux'}[platform.system()],
                'runtime_cases': {}, 'binaries': {}}
    if platform.system() == 'Darwin':
        metadata['hardware'] = command(['/usr/sbin/sysctl', 'machdep.cpu.brand_string', 'hw.memsize'], env=None)[1].decode()
    elif Path('/proc/cpuinfo').is_file():
        metadata['hardware'] = '\n'.join(line for line in Path('/proc/cpuinfo').read_text().splitlines()
                                         if line.startswith(('model name', 'Hardware', 'MemTotal')))[:4096]
    if args.c_compiler:
        c_compiler = args.c_compiler.resolve(strict=True)
        metadata['c_reference'] = {'compiler': str(c_compiler), 'sha256': digest(c_compiler),
                                   'flags': ['-std=c11', '-O3', '-ftrapv'],
                                   'version': command([c_compiler, '--version'], env=None)[1].decode(),
                                   'caveat': 'C has no Dunder array bounds checks; no broad parity claim'}
    rows = []

    def sample(suite, case, variant, round_number, position, elapsed, binary):
        rows.append({'suite': suite, 'case': case, 'variant': variant,
                     'round': round_number, 'order': position,
                     'elapsed_ms': f'{elapsed:.6f}',
                     'image_bytes': binary.stat().st_size if binary else 0})

    with tempfile.TemporaryDirectory(prefix='dunder-performance-') as tmp:
        work = Path(tmp)
        if args.suite in ('all', 'runtime'):
            for name, inputs, result in runtime_cases():
                expected = f'{result}\n'.encode()
                metadata['runtime_cases'][name] = {'args': inputs, 'expected': expected.decode()}
                binaries = {}
                for variant, compiler in compilers.items():
                    binary = work / f'{name}-{variant}'
                    command([compiler, 'build', ROOT / f'benchmarks/runtime/{name}.ddr', '-o', binary, *compiler_flags[variant]])
                    binaries[variant] = binary
                if args.c_compiler:
                    binary = work / f'{name}-c'
                    command([c_compiler, '-std=c11', '-O3', '-ftrapv', f'-DCASE_{name}',
                             ROOT / 'benchmarks/runtime/reference.c', '-o', binary], env=None)
                    binaries['c'] = binary
                metadata['binaries'][name] = {v: {'sha256': digest(p), 'bytes': p.stat().st_size}
                                               for v, p in binaries.items()}
                for binary in binaries.values():
                    checked_run(binary, inputs, expected)
                for round_number in range(args.rounds):
                    for position, variant in enumerate(order(list(binaries), round_number)):
                        elapsed = checked_run(binaries[variant], inputs, expected)
                        sample('runtime', name, variant, round_number, position, elapsed, binaries[variant])
                print(f'completed runtime/{name}', flush=True)
        if args.suite in ('all', 'compiler'):
            stress, metadata['generated_corpus'] = corpus(work)
            entry = {'Darwin': 'main', 'Linux': 'linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_linux'}[platform.system()]
            for name, source in [('generated', stress), ('selfhost', ROOT / f'selfhost/{entry}.ddr'),
                                 ('tabell', ROOT / 'projects/tabell/main.ddr')]:
                for round_number in range(args.rounds):
                    for position, variant in enumerate(order(list(compilers), round_number)):
                        binary = work / f'{name}-{variant}-{round_number}'
                        elapsed, _ = command([compilers[variant], 'build', source, '-o', binary, *compiler_flags[variant]])
                        if name == 'generated':
                            checked_run(binary, [], b'595500\n')
                        elif name == 'selfhost':
                            hello = work / 'hello'
                            command([binary, 'build', ROOT / 'examples/hello.ddr', '-o', hello])
                            checked_run(hello, [], b'42\ntrue\n')
                        else:
                            checked_run(binary, [ROOT / 'projects/tabell/expenses.csv', 'team', 'cents'],
                                        (ROOT / 'projects/tabell/expected.csv').read_bytes())
                        sample('compiler', name, variant, round_number, position, elapsed, binary)
                        image = {'sha256': digest(binary), 'bytes': binary.stat().st_size}
                        previous = metadata['binaries'].setdefault(name, {}).setdefault(variant, image)
                        if previous != image:
                            raise RuntimeError(f'non-reproducible {name} output from {variant}')
                        binary.unlink()
                print(f'completed compiler/{name}', flush=True)
                if args.include_check:
                    for round_number in range(args.rounds):
                        for position, variant in enumerate(order(list(compilers), round_number)):
                            elapsed, output = command([compilers[variant], 'check', source, *compiler_flags[variant]])
                            expected_prefix = f'checked {source} ('.encode()
                            if not output.startswith(expected_prefix) or not output.endswith(b' modules)\n'):
                                raise RuntimeError(f'invalid check result from {variant}: {output!r}')
                            sample('check', name, variant, round_number, position, elapsed, None)
                    print(f'completed check/{name}', flush=True)
    if source_hashes() != before:
        raise RuntimeError('benchmark source files changed during measurement')
    if any(digest(p) != metadata['compiler_sha256'][v] for v, p in compilers.items()):
        raise RuntimeError('compiler binary changed during measurement')
    summaries = []
    for suite, case in dict.fromkeys((r['suite'], r['case']) for r in rows):
        baseline = statistics.median(float(r['elapsed_ms']) for r in rows
                                     if (r['suite'], r['case'], r['variant']) == (suite, case, 'baseline'))
        for variant in dict.fromkeys(r['variant'] for r in rows if (r['suite'], r['case']) == (suite, case)):
            samples = [r for r in rows if (r['suite'], r['case'], r['variant']) == (suite, case, variant)]
            times = sorted(float(r['elapsed_ms']) for r in samples)
            median, p95 = statistics.median(times), times[math.ceil(len(times) * .95) - 1]
            summaries.append({'suite': suite, 'case': case, 'variant': variant, 'samples': len(times),
                              'median_ms': f'{median:.6f}', 'p95_ms': f'{p95:.6f}',
                              'median_over_baseline': f'{median / baseline:.6f}',
                              'p95_lines_per_second': f"{metadata['generated_corpus']['lines'] * 1000 / p95:.0f}"
                              if suite == 'compiler' and case == 'generated' else '',
                              'image_bytes': samples[0]['image_bytes']})
    save_csv(report / 'samples.csv', rows)
    save_csv(report / 'summary.csv', summaries)
    (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + '\n')
    for summary in summaries:
        print(f"{summary['suite']}/{summary['case']} {summary['variant']}: "
              f"median={summary['median_ms']}ms p95={summary['p95_ms']}ms "
              f"ratio={summary['median_over_baseline']}")


if __name__ == '__main__':
    main()
