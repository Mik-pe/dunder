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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('baseline', 'candidate', 'source_root', 'tabell', 'report'):
        parser.add_argument(key, type=Path)
    parser.add_argument('--rounds', type=int, default=12)
    args = parser.parse_args()
    if args.rounds < 1 or args.report.exists():
        parser.error('use a positive number of rounds and a new report path')
    compilers = {v: getattr(args, v).resolve(strict=True) for v in ('baseline', 'candidate')}
    root = args.source_root.resolve(strict=True)
    tabell = args.tabell.resolve(strict=True)
    rows, images, corpus_ids = [], {}, {}
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        generated, corpus = performance.corpus(work)
        cases = {'generated': generated, 'unique': make_unique(work, generated, False),
                 'mixed': make_unique(work, generated, True),
                 'selfhost': root / 'selfhost/main_parallel.ddr', 'tabell': tabell / 'main.ddr'}
        expected = {'generated': b'595500\n', 'unique': b'1095000\n', 'mixed': b'24000\n',
                    'tabell': (tabell / 'expected.csv').read_bytes()}
        for case, source in cases.items():
            if case in ('generated', 'unique', 'mixed'):
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
                    assert images.setdefault(key, digest) == digest
                    if i == -1:
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
        args.report.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
