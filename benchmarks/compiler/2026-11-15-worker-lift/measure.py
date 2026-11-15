"""Paired native build latency on fixed sources. See README.md for invocation."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import statistics
import subprocess
import time
import sys
import tempfile

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'tools'))
import performance


def make_unique(work, generated, mixed):
    directory = work / ('mixed' if mixed else 'unique')
    directory.mkdir()
    for source in generated.parent.glob('*.ddr'):
        text = source.read_text()
        if source.name != 'main.ddr':
            module = int(source.stem[1:])
            assignments = iter(range(50 * 96))
            def change(_match):
                index = next(assignments)
                function = module * 50 + index // 96
                if not mixed:
                    value = function + 1 if index % 96 == 0 else 1
                    return f'value = value + {value}'
                if index % 96 == 1:
                    return f'value = value - {function + 1}'
                return ('value = value + x', 'value = value - 1',
                        'value = value + 2', 'value = value - x')[index % 4]
            text, count = re.subn(r'value = value \+ 1', change, text)
            assert count == 50 * 96
        (directory / source.name).write_text(text)
    return directory / 'main.ddr'


def large_function(work):
    # Keep one value live through 4,096 dependent updates in a single CFG.
    directory = work / 'large-function'
    directory.mkdir()
    source = directory / 'main.ddr'
    lines = ['fn compute(x: i64) -> i64 {', '    var value := x']
    x, value = 17, 17
    for index in range(4096):
        lines.append(f'    value = (value * 3 + x + {index}) % 1000003')
        value = (value * 3 + x + index) % 1000003
    lines.extend(['    value', '}', f'fn main() {{ print(compute({x})) }}'])
    source.write_text('\n'.join(lines) + '\n')
    return source, f'{value}\n'.encode()


def varied_calls(work):
    # Every group has distinct call targets, owned aggregates and borrowed
    # mutation. Select both enum payload variants across the complete corpus.
    directory = work / 'varied-calls'
    directory.mkdir()
    source = directory / 'main.ddr'
    lines = ['struct Item { label: string, values: []i64 }',
             'enum Reading { Count(i64), Text(string) }',
             'fn score(reading: Reading) -> i64 {',
             '    match reading { .Count(n) => n, .Text(s) => i64(s.len) }',
             '}']
    calls = []
    expected = 0
    for index in range(256):
        label = f'item{index}'
        lines.extend([
            f'fn item{index}(n: i64) -> Item {{',
            f'    Item {{ label: "{label}", values: [n, n + {index}] }}',
            '}',
            f'fn update{index}(a: &mut []i64, text: string) -> Reading {{',
            f'    a[0] = a[0] + {index + 1}',
            '    if a[0] % 3 == 0 { return Reading.Text(text) }',
            '    Reading.Count(a[1])',
            '}',
            f'fn visit{index}(item: Item) -> i64 {{',
            '    var a := item.values',
            f'    reading := update{index}(&mut a, item.label)',
            '    score(reading) + a[0] + item.values[0]',
            '}'])
        n = index + 7
        calls.append(f'    total = total + visit{index}(item{index}({n}))')
        updated = n + index + 1
        score = len(label) if updated % 3 == 0 else n + index
        expected += score + updated + n
    lines.extend(['fn main() {', '    var total: i64 = 0', *calls, '    print(total)', '}'])
    source.write_text('\n'.join(lines) + '\n')
    return source, f'{expected}\n'.encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('baseline', 'candidate', 'source_root', 'tabell', 'report'):
        parser.add_argument(key, type=Path)
    parser.add_argument('--rounds', type=int, default=12)
    parser.add_argument('--frontend-stress', action='store_true',
                        help='also build one large function and a many-function typed call graph')
    parser.add_argument('--rss-rounds', type=int, default=0,
                        help='run separate paired builds under macOS time -l for peak RSS')
    args = parser.parse_args()
    if args.rounds < 1 or args.report.exists():
        parser.error('use a positive number of rounds and a new report path')
    if args.rss_rounds < 0 or (args.rss_rounds and platform.system() != 'Darwin'):
        parser.error('--rss-rounds requires a nonnegative count and macOS for measured RSS')
    compilers = {v: getattr(args, v).resolve(strict=True) for v in ('baseline', 'candidate')}
    root = args.source_root.resolve(strict=True)
    tabell = args.tabell.resolve(strict=True)
    rows, rss_samples, images, corpus_ids = [], [], {}, {}
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        generated, corpus = performance.corpus(work)
        cases = {'generated': generated, 'unique': make_unique(work, generated, False),
                 'mixed': make_unique(work, generated, True),
                 'selfhost': root / 'selfhost/main_parallel.ddr', 'tabell': tabell / 'main.ddr'}
        expected = {'generated': b'595500\n', 'unique': b'1095000\n', 'mixed': b'24000\n',
                    'tabell': (tabell / 'expected.csv').read_bytes()}
        synthetic = {'generated', 'unique', 'mixed'}
        if args.frontend_stress:
            for name, generator in [('large-function', large_function), ('varied-calls', varied_calls)]:
                cases[name], expected[name] = generator(work)
                synthetic.add(name)
        for case, source in cases.items():
            if case in synthetic:
                inputs = sorted(source.parent.glob('*.ddr'))
                corpus_ids[case] = hashlib.sha256(b''.join(p.read_bytes() for p in inputs)).hexdigest()
            for i in range(-1, args.rounds):
                for variant in performance.order(list(compilers), i):
                    binary = work / 'program'
                    binary.unlink(missing_ok=True)
                    start = time.perf_counter_ns()
                    result = subprocess.run([compilers[variant], 'build', source, '--jobs=4',
                                             '--timings=json', '-o', binary], cwd=root,
                                            env=performance.ENV, capture_output=True, check=True, timeout=120)
                    ms = (time.perf_counter_ns() - start) / 1e6
                    timings = json.loads(result.stderr)
                    key = f'{case}-{variant}'
                    digest = performance.digest(binary)
                    if images.setdefault(key, digest) != digest:
                        raise RuntimeError(f'non-reproducible image for {key}')
                    if i == -1:
                        if args.frontend_stress:
                            serial = work / 'one-worker'
                            performance.command([compilers[variant], 'build', source,
                                                 '--jobs=1', '-o', serial], cwd=root)
                            if performance.digest(serial) != digest:
                                raise RuntimeError(f'worker count changed image for {key}')
                        if case == 'selfhost':
                            hello = work / 'hello'
                            performance.command([binary, 'build', root / 'examples/hello.ddr', '-o', hello], cwd=root)
                            performance.checked_run(hello, [], b'42\ntrue\n')
                        else:
                            argv = [tabell / 'expenses.csv', 'team', 'cents'] if case == 'tabell' else []
                            performance.checked_run(binary, argv, expected[case])
                        continue
                    phases = timings['phases']
                    before_image = sum(phases[p] for p in ('module_load_lex_resolve', 'build_setup', 'compiler'))
                    rows.append(dict(case=case, variant=variant, round=i+1, wall_ms=round(ms, 3),
                                     pre_image_ms=before_image / 1e6, mir_operations=timings['mir']['operations'],
                                     image_bytes=binary.stat().st_size))
            for i in range(args.rss_rounds):
                for variant in performance.order(list(compilers), i):
                    binary = work / 'program'
                    binary.unlink(missing_ok=True)
                    result = subprocess.run(['/usr/bin/time', '-l', compilers[variant], 'build', source,
                                             '--jobs=4', '--timings=json', '-o', binary],
                                            cwd=root, env=performance.ENV, capture_output=True, check=True,
                                            timeout=120)
                    match = re.search(rb'^\s*(\d+)\s+maximum resident set size$', result.stderr, re.M)
                    if not match or performance.digest(binary) != images[f'{case}-{variant}']:
                        raise RuntimeError(f'RSS observation or image mismatch for {case}/{variant}')
                    rss_samples.append(dict(case=case, variant=variant, round=i + 1,
                                            max_rss_bytes=int(match.group(1))))
            summary = [round(statistics.median(r['wall_ms'] for r in rows if r['case']==case
                        and r['variant']==v), 2) for v in compilers]
            print(case, *summary, flush=True)
        tabell_hashes = {p.name: performance.digest(p) for p in sorted(tabell.glob('*.ddr'))}
        revision = None
        if (root / '.git').exists():
            revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
        report = dict(target=platform.platform(), rounds=args.rounds, source_revision=revision,
                      compiler_sha256={v: performance.digest(p) for v,p in compilers.items()},
                      corpus=corpus, corpus_sha256=corpus_ids, tabell_sha256=tabell_hashes,
                      image_sha256=images, samples=rows)
        if args.rss_rounds:
            report['rss_clock'] = 'independent fresh builds via macOS /usr/bin/time -l; maximum resident set size in bytes'
            report['rss_rounds'] = args.rss_rounds
            report['rss_samples'] = rss_samples
        if args.frontend_stress:
            report['build_jobs'] = 4
            report['checked_identical_jobs'] = [1, 4]
            report['generated_expected_stdout'] = {k: expected[k].decode() for k in sorted(synthetic)}
            report['selfhost_sources_sha256'] = {
                str(p.relative_to(root)): performance.digest(p)
                for section in ('selfhost', 'stdlib') for p in sorted((root / section).glob('*.ddr'))
            }
            report['tabell_sha256'].update({p: performance.digest(tabell / p)
                                            for p in ('expenses.csv', 'expected.csv')})
        args.report.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
