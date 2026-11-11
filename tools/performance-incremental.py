#!/usr/bin/env python3
"""Compare fresh-process warm/incremental native builds with isolated caches.

Recorded commands use --incremental and no profiling/timing instrumentation.
Nonincremental reference builds, correctness execution, cache priming and JSON
counter observations are outside every recorded interval. This is neither a
cold-build nor a runtime benchmark.
"""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
import performance as perf

ROOT = Path(__file__).resolve().parents[1]
VARIANTS = ('baseline', 'candidate')
PHASES = ('module_load_lex_resolve', 'build_setup', 'compiler', 'image_format_sign', 'publication')


def probe_sources():
    private = {}
    for state, delta in (('A', 1), ('B', 2)):
        private[state] = {
            'calc.ddr': f'fn inc(x:i32)->i32{{x+{delta}}}\npub fn consume(x:i32)->i32{{inc(x)+inc(x+1)}}\n',
            'main.ddr': 'import calc\nfn main(){var total:=0;for i in 0..20{total=total+calc.consume(i)};print(total)}\n',
        }
    layout = {}
    for state, fields in (('A', 'left:i64,right:i64'), ('B', 'right:i64,left:i64')):
        layout[state] = {
            'catalog.ddr': 'pub struct Pair{' + fields + '}\n'
                           'pub fn make()->Pair{Pair{left:11,right:7}}\n'
                           'pub fn weight(value:&Pair)->i64{value.left*100+value.right}\n',
            'main.ddr': 'import catalog\nfn main(){value:=catalog.make();print(catalog.weight(&value));print(value.left);print(value.right)}\n',
        }
    return private, layout


def state_hash(files):
    value = hashlib.sha256()
    for name, content in sorted(files.items()):
        value.update(name.encode()); value.update(b'\0'); value.update(content.encode()); value.update(b'\0')
    return value.hexdigest()


def timings(stderr, target):
    data = json.loads(stderr)
    assert (data['kind'], data['version'], data['operation'], data['scope']) == ('timings', 1, 'build', 'compiler_pipeline'), data
    assert data['clock'] == 'monotonic' and data['clock_ok'] is True and data['unit'] == 'ns' and data['target'] == target, data
    assert data['skipped_phases'] == [] and set(data['phases']) == set(PHASES), data
    assert all(type(data['phases'][p]) is int and data['phases'][p] >= 0 for p in PHASES), data
    assert type(data['total']) is int and data['total'] == sum(data['phases'].values()), data
    assert all(type(data['functions'][key]) is int and data['functions'][key] >= 0 for key in ('reused', 'emitted')), data
    return data


def cache_info(cwd):
    files = list((cwd / 'build/.dunder-cache').rglob('*.fn'))
    result = dict(files=len(files), bytes=0, native3=0, dufd=0)
    for path in files:
        result['bytes'] += path.stat().st_size
        with path.open('rb') as file:
            prefix = file.read(136)
        result['dufd' if prefix[128:136] == b'44554644' else 'native3'] += 1
    return result


def readme():
    return '''# Warm and incremental compiler latency

Each recorded sample starts a fresh compiler process and includes source
loading, incremental cache IO, image formatting/signing and publication.
The OS filesystem cache is warm. Variants have separate working directories,
fragment caches and TMPDIRs, and use the same frozen absolute source paths.
No runtime, cold-build or general C-performance claim follows from this suite.

Generated, selfhost and Tabell inputs remain unchanged. The two small probes
change only a private inlineable helper body or a shared record's field layout.
Names and call graphs stay fixed. After priming state A, edit rounds start with
B and alternate B/A. Edit summaries combine both transition directions; state
and image hashes remain available in the raw samples.

Every image must match an untimed nonincremental reference for its compiler,
flags and source state. Generated output is independently 595500; selfhost
images build/run hello (42, true); Tabell is checked against both Python CSV
aggregation and its frozen golden file. Private-body outputs are independently
440/480; layout outputs remain 1107, 11, 7 through named construction.

Two JSON timing builds prime each measured cache (cold then warm), outside
recorded intervals. Reused/emitted counters are out-of-band only. After the
latency series, separate observer caches replay priming and representative
A/B/A transitions, so observing an edit never warms a measured transition.
These observer counts are not per-recorded-round counts or cold latency data.
Correctness execution and output hashing also occur after each clock stops.

Raw times use perf_counter_ns around subprocess.run, including process startup.
Two variants alternate order each round. No sample is removed. Median uses the
middle pair for even counts; p95 is nearest rank ceil(0.95*N). Profile/timing
flags are forbidden in recorded variant flags. All safety settings otherwise
come from the selected binaries and explicit flags in metadata.json.

The corpus and repeated small probes are finite workloads. Peak memory, cold
OS caches, editor responsiveness and optimization quality are not measured.
Nonincremental reference/validation builds have no reported latency here.
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('baseline', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('report', type=Path)
    parser.add_argument('--rounds', type=int, default=30)
    parser.add_argument('--baseline-flag', action='append', default=[])
    parser.add_argument('--candidate-flag', action='append', default=[])
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error('--rounds must be positive')
    compilers = {'baseline': args.baseline.resolve(strict=True), 'candidate': args.candidate.resolve(strict=True)}
    flags = {'baseline': args.baseline_flag, 'candidate': args.candidate_flag}
    forbidden = ('--timings', '--profile-compiler', '--runtime-profile', '--incremental', '--target', '--app', '--agent', '--run', '--output')
    for variant, extra in flags.items():
        for flag in extra:
            if not flag.startswith('--') or flag.split('=')[0] in forbidden:
                parser.error(f'{variant} flag conflicts with fixed input/target/instrumentation scope: {flag}')
    report = args.report.resolve()
    report.mkdir(parents=True, exist_ok=True)
    if any(report.iterdir()):
        parser.error('report directory must be empty to preserve measurements')
    if platform.system() == 'Darwin' and platform.machine() == 'arm64':
        target, entry = 'arm64-macos', 'main'
    elif platform.system() == 'Linux' and platform.machine() in ('arm64', 'aarch64', 'x86_64'):
        target, entry = ('arm64-linux', 'linux') if platform.machine() in ('arm64', 'aarch64') else ('x86_64-linux', 'x86_linux')
    else:
        parser.error('native execution requires ARM64 macOS or ARM64/x86-64 Linux')
    sources_before = perf.source_hashes()
    hello_hash = perf.digest(ROOT / 'examples/hello.ddr')
    metadata = dict(complete=False, rounds=args.rounds, target=target, host=platform.uname()._asdict(),
                    sources_sha256=sources_before, compiler_paths={v: str(p) for v, p in compilers.items()},
                    compiler_sha256={v: perf.digest(p) for v, p in compilers.items()}, compiler_flags=flags,
                    tool_sha256=perf.digest(Path(__file__)), helpers_sha256=perf.digest(ROOT / 'tools/performance.py'),
                    clock='perf_counter_ns around fresh subprocess.run; milliseconds',
                    environment=dict(PATH='/no-external-tools', TMPDIR='isolated directory per cwd, under temporary work root'),
                    cache='warm OS filesystem; incremental per-variant/per-case cache; two unrecorded JSON primes',
                    order='baseline/candidate alternate each round; same frozen input paths',
                    counts='out-of-band only; separate post-series representative observer replay',
                    embedded_inputs='compiler-embedded runtime/library versions are identified by each compiler hash',
                    commands=[], cases={})
    samples = []

    def checkpoint():
        perf.save_csv(report / 'samples.csv', samples)
        (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + '\n')

    def run(command, cwd, measured=False, purpose='validation'):
        command = [str(value) for value in command]
        temporary = cwd / 'tmp'; temporary.mkdir(parents=True, exist_ok=True)
        env = dict(PATH='/no-external-tools', TMPDIR=str(temporary) + '/')
        started = time.perf_counter_ns() if measured else None
        result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, timeout=120)
        elapsed = (time.perf_counter_ns() - started) / 1_000_000 if measured else None
        metadata['commands'].append(dict(command=command, cwd=str(cwd), environment=env, purpose=purpose,
                                         measured=measured, status=result.returncode,
                                         elapsed_ms=elapsed,
                                         stdout=result.stdout.decode(errors='replace'), stderr=result.stderr.decode(errors='replace')))
        if result.returncode:
            raise RuntimeError(f'{purpose}: exit {result.returncode}: {command}\n{result.stdout!r}\n{result.stderr!r}')
        return result, elapsed

    try:
        metadata['checkout'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        metadata['working_tree'] = subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True)
        if platform.system() == 'Darwin':
            metadata['hardware'] = subprocess.check_output(['/usr/sbin/sysctl', 'machdep.cpu.brand_string', 'hw.memsize'], text=True)
        else:
            metadata['hardware'] = '\n'.join(line for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith(('model name', 'Hardware')))[:4096]
            metadata['memory'] = Path('/proc/meminfo').read_text().splitlines()[0]
        with tempfile.TemporaryDirectory(prefix='dunder-incremental-performance-') as directory:
            work = Path(directory).resolve(); inputs = work / 'inputs'; inputs.mkdir()
            snapshot = metadata['input_snapshot_sha256'] = {}
            # Copy only frozen inputs, never an existing project build/cache.
            for relative in sources_before:
                if relative.startswith(('selfhost/', 'stdlib/', 'runtime/', 'projects/tabell/')):
                    destination = inputs / relative; destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(ROOT / relative, destination)
                    snapshot[relative] = perf.digest(destination)
                    assert snapshot[relative] == sources_before[relative], ('input changed while copying', relative)
            hello = inputs / 'hello.ddr'; hello.write_bytes((ROOT / 'examples/hello.ddr').read_bytes())
            metadata['hello_sha256'] = perf.digest(hello)
            assert metadata['hello_sha256'] == hello_hash
            snapshot['hello.ddr'] = hello_hash
            stress, corpus_info = perf.corpus(inputs)
            metadata['generated_corpus'] = corpus_info
            for path in stress.parent.glob('*.ddr'):
                snapshot[str(path.relative_to(inputs))] = perf.digest(path)
            tabell = inputs / 'projects/tabell'
            with (tabell / 'expenses.csv').open(newline='') as file:
                totals = {}
                for row in csv.DictReader(file):
                    totals[row['team']] = totals.get(row['team'], 0) + int(row['cents'])
            tabell_expected = ('group,total\n' + ''.join(f'{name},{totals[name]}\n' for name in sorted(totals))).encode()
            assert tabell_expected == (tabell / 'expected.csv').read_bytes(), 'Tabell golden differs from independent CSV reference'
            private, layout = probe_sources()
            cases = [dict(name='generated_unchanged', source=stress, kind='generated', states={'A': {}}, expected={'A': b'595500\n'}),
                     dict(name='selfhost_unchanged', source=inputs / f'selfhost/{entry}.ddr', kind='selfhost', states={'A': {}}, expected={'A': b'42\ntrue\n'}),
                     dict(name='tabell_unchanged', source=tabell / 'main.ddr', kind='tabell', states={'A': {}}, expected={'A': tabell_expected}),
                     dict(name='private_body_edit', source=inputs / 'private/main.ddr', kind='probe', states=private,
                          expected={state: f'{sum((i+d)+(i+1+d) for i in range(20))}\n'.encode() for state, d in (('A', 1), ('B', 2))}),
                     dict(name='shared_layout_edit', source=inputs / 'layout/main.ddr', kind='probe', states=layout,
                          expected={'A': b'1107\n11\n7\n', 'B': b'1107\n11\n7\n'})]

            for case in cases:
                name, source = case['name'], case['source']
                case_meta = metadata['cases'][name] = dict(kind=case['kind'], source=str(source), states={}, references={}, primes={}, observers={})
                for state, files in case['states'].items():
                    case_meta['states'][state] = dict(files=files, edited_source_sha256=state_hash(files), expected_stdout=case['expected'][state].decode())

                def install(state):
                    for relative, content in case['states'][state].items():
                        path = source.parent / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(content)

                def cwd_for(group, variant):
                    cwd = work / group / variant / name; cwd.mkdir(parents=True, exist_ok=True); return cwd

                def validate(image, variant, state):
                    cwd = cwd_for('validation', variant)
                    if case['kind'] == 'selfhost':
                        program = cwd / 'hello'
                        if program.exists():
                            program.unlink()
                        run([image, 'build', hello, '-o', program], cwd, purpose=f'{name}/{variant}/{state}/hello-build')
                        result, _ = run([program], cwd, purpose=f'{name}/{variant}/{state}/hello-run')
                    else:
                        arguments = [tabell / 'expenses.csv', 'team', 'cents'] if case['kind'] == 'tabell' else []
                        result, _ = run([image, *arguments], cwd, purpose=f'{name}/{variant}/{state}/program-run')
                    assert result.stdout == case['expected'][state], (name, variant, state, result.stdout, case['expected'][state])

                def build(variant, state, group, incremental, observer=False, measured=False):
                    cwd = cwd_for(group, variant); image = cwd / 'image'
                    if image.exists():
                        image.unlink()
                    command = [compilers[variant], 'build', source, '--target', target, '-o', image, *flags[variant]]
                    if incremental:
                        command.append('--incremental')
                    if observer:
                        command.append('--timings=json')
                    result, elapsed = run(command, cwd, measured, f'{name}/{variant}/{state}/{group}')
                    if not observer:
                        assert result.stderr == b'', (name, variant, state, 'unexpected instrumentation/diagnostic', result.stderr)
                    image_info = dict(sha256=perf.digest(image), bytes=image.stat().st_size)
                    if group != 'references':
                        assert image_info == case_meta['references'][variant][state], (name, variant, state, group, 'incremental/nonincremental image mismatch')
                    validate(image, variant, state)
                    return image_info, elapsed, timings(result.stderr, target) if observer else None

                # Each source-state reference is a normal build with no cache.
                for state in case['states']:
                    install(state)
                    for variant in VARIANTS:
                        image_info, _, _ = build(variant, state, 'references', False)
                        case_meta['references'].setdefault(variant, {})[state] = image_info
                install('A')
                for variant in VARIANTS:
                    records = []
                    for label in ('cold_prime', 'warm_prime'):
                        _, _, observed = build(variant, 'A', 'measured', True, observer=True)
                        if label == 'cold_prime':
                            assert observed['functions']['reused'] == 0, (name, variant, 'cache was not empty')
                        else:
                            assert observed['functions']['reused'] > 0, (name, variant, 'no warm fragment reuse')
                        records.append(dict(label=label, observation=observed))
                    case_meta['primes'][variant] = dict(observations=records, cache=cache_info(cwd_for('measured', variant)))
                    assert case_meta['primes'][variant]['cache']['files'] > 0
                checkpoint()

                for round_number in range(args.rounds):
                    state = ('B' if round_number % 2 == 0 else 'A') if 'B' in case['states'] else 'A'
                    install(state)
                    for position, variant in enumerate(perf.order(list(VARIANTS), round_number)):
                        image_info, elapsed, _ = build(variant, state, 'measured', True, measured=True)
                        samples.append(dict(suite='incremental', case=name, variant=variant, round=round_number,
                                            order=position, state=state, elapsed_ms=f'{elapsed:.6f}',
                                            image_bytes=image_info['bytes'], image_sha256=image_info['sha256']))
                checkpoint()

                # Counter observation uses separate caches AFTER the timed
                # series, never priming a measured source transition.
                for variant in VARIANTS:
                    records = []
                    for label, state in [('cold_prime', 'A'), ('warm_unchanged', 'A')] + (
                            [('edit_A_to_B', 'B'), ('edit_B_to_A', 'A')] if 'B' in case['states'] else []):
                        install(state)
                        _, _, observed = build(variant, state, 'observers', True, observer=True)
                        records.append(dict(label=label, state=state, observation=observed))
                    case_meta['observers'][variant] = dict(observations=records, cache=cache_info(cwd_for('observers', variant)))
                install('A')
                checkpoint()
                print('completed incremental/' + name, flush=True)
        assert perf.source_hashes() == sources_before, 'repository source changed during measurement'
        assert perf.digest(ROOT / 'examples/hello.ddr') == hello_hash, 'hello source changed during measurement'
        assert perf.digest(Path(__file__)) == metadata['tool_sha256'] and perf.digest(ROOT / 'tools/performance.py') == metadata['helpers_sha256'], 'measurement tool/helper changed'
        assert all(perf.digest(path) == metadata['compiler_sha256'][variant] for variant, path in compilers.items()), 'compiler changed during measurement'
        summaries = []
        for name in metadata['cases']:
            base = statistics.median(float(row['elapsed_ms']) for row in samples if row['case'] == name and row['variant'] == 'baseline')
            for variant in VARIANTS:
                selected = [row for row in samples if row['case'] == name and row['variant'] == variant]
                times = sorted(float(row['elapsed_ms']) for row in selected)
                median, p95 = statistics.median(times), times[math.ceil(len(times) * .95) - 1]
                summaries.append(dict(suite='incremental', case=name, variant=variant, samples=len(times),
                                      median_ms=f'{median:.6f}', p95_ms=f'{p95:.6f}', median_over_baseline=f'{median/base:.6f}',
                                      image_bytes=','.join(map(str, sorted({row['image_bytes'] for row in selected})))))
        perf.save_csv(report / 'summary.csv', summaries)
        metadata['complete'] = True
        checkpoint()
        (report / 'README.md').write_text(readme())
        for row in summaries:
            print(f"{row['case']} {row['variant']}: median={row['median_ms']}ms p95={row['p95_ms']}ms ratio={row['median_over_baseline']}")
    except Exception as error:
        metadata['error'] = repr(error)
        checkpoint()
        raise


if __name__ == '__main__':
    main()
