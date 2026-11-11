#!/usr/bin/env python3
"""Attribute cold/warm fragment work with separate clock/allocation observers.

No external wall clock or latency comparison is collected. Cold means an empty
fragment cache, with a warm OS filesystem cache. Clock and heap observers have
separate compiler images, processes, working directories and caches.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH': '/no-external-tools'}


def helper(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools' / filename)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


PERF = helper('cache_performance', 'performance.py')
PROFILE = helper('cache_compiler_profile', 'profile-compiler.py')
CACHE = helper('cache_stage_validation', 'test-cache-meter.py')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('builder', type=Path, help='frozen normal compiler, used to build both observer compilers')
    parser.add_argument('report', type=Path)
    parser.add_argument('--rounds', type=int, default=5)
    parser.add_argument('--case', action='append', choices=('generated', 'selfhost', 'tabell'))
    parser.add_argument('--compiler-flag', action='append', default=[], help='workload flag; default --inline-opt=off')
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error('--rounds must be positive')
    flags = args.compiler_flag or ['--inline-opt=off']
    for flag in flags:
        if not flag.startswith('--') or flag.split('=')[0] in ('--timings', '--profile-compiler', '--runtime-profile', '--incremental', '--target', '--app', '--output'):
            parser.error('observer/target flags are fixed: ' + flag)
    builder = args.builder.resolve(strict=True)
    report = args.report.resolve(); report.mkdir(parents=True, exist_ok=True)
    if any(report.iterdir()):
        parser.error('report directory must be empty')
    target, entry = PROFILE.host()
    sources_before = PERF.source_hashes()
    hashes = dict(sources_before)
    for path in ('tools/profile-cache.py', 'tools/test-cache-meter.py', 'tools/profile-compiler.py', 'tools/profile-dogfood.py', 'tools/performance.py', 'examples/hello.ddr'):
        hashes[path] = PERF.digest(ROOT / path)
    metadata = dict(complete=False, rounds=args.rounds, host=platform.uname()._asdict(), target=target,
                    source_sha256=hashes, builder=str(builder), builder_sha256=PERF.digest(builder),
                    observer_build_flags=['--inline-opt=off'], compiler_flags=flags, environment=ENV,
                    checkout=subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip(),
                    working_tree=subprocess.check_output(['git','status','--short'], cwd=ROOT, text=True),
                    scope='cache attribution only; empty-fragment-cache cold versus warm, warm OS cache; no wall latency',
                    overlap='cache_detail overlaps fragment_cache and is excluded from the 11-phase covered sum',
                    allocation_scope='executed allocator/RC/copy helper traffic, not live/peak memory; clocks disabled',
                    commands=[], cases={}, observations=[])
    if platform.system() == 'Darwin':
        metadata['hardware'] = subprocess.check_output(['/usr/sbin/sysctl','machdep.cpu.brand_string','hw.memsize'], text=True)
    else:
        metadata['hardware'] = Path('/proc/cpuinfo').read_text()[:8192]
        metadata['memory'] = Path('/proc/meminfo').read_text().splitlines()[0]

    def checkpoint():
        (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + '\n')

    def run(command, cwd, label):
        result = subprocess.run([str(value) for value in command], cwd=cwd, env=ENV,
                                capture_output=True, timeout=120)
        stem = f'{len(metadata["commands"]):04}-{label}'
        (report / (stem + '.stdout')).write_bytes(result.stdout)
        (report / (stem + '.stderr')).write_bytes(result.stderr)
        metadata['commands'].append(dict(command=[str(x) for x in command], cwd=str(cwd), label=label,
                                         status=result.returncode, stdout=stem+'.stdout', stderr=stem+'.stderr'))
        if result.returncode:
            raise RuntimeError((label, result.returncode, result.stdout, result.stderr))
        return result

    try:
        with tempfile.TemporaryDirectory(prefix='dunder-cache-profile-') as directory:
            work = Path(directory).resolve()
            source = ROOT / f'selfhost/{entry}.ddr'
            clock_compiler, allocation_compiler = report / 'clock-compiler', report / 'allocation-compiler'
            run([builder,'build',source,'-o',clock_compiler,'--inline-opt=off'], work, 'build-clock-compiler')
            run([builder,'build',source,'-o',allocation_compiler,'--inline-opt=off','--runtime-profile'], work, 'build-allocation-compiler')
            metadata['observers'] = {name: dict(path=str(path), sha256=PERF.digest(path), bytes=path.stat().st_size)
                                     for name,path in (('clock',clock_compiler),('allocation',allocation_compiler))}
            generated, corpus = PERF.corpus(work)
            metadata['corpus'] = corpus
            all_cases = [('generated',generated),('selfhost',source),('tabell',ROOT/'projects/tabell/main.ddr')]
            selected = set(args.case or [name for name,_ in all_cases])

            def validate(image, case, cwd):
                if case == 'selfhost':
                    hello = cwd / 'hello'
                    run([image,'build',ROOT/'examples/hello.ddr','-o',hello], cwd, case+'-hello-build')
                    result = run([hello], cwd, case+'-hello-run'); expected = b'42\ntrue\n'
                elif case == 'generated':
                    result = run([image], cwd, case+'-run'); expected = b'595500\n'
                else:
                    result = run([image,ROOT/'projects/tabell/expenses.csv','team','cents'], cwd, case+'-run')
                    expected = (ROOT/'projects/tabell/expected.csv').read_bytes()
                assert result.stdout == expected, (case, result.stdout, expected)

            for case, source in all_cases:
                if case not in selected:
                    continue
                cwd = work / ('reference-'+case); cwd.mkdir()
                reference = cwd / 'image'
                run([builder,'build',source,'-o',reference,*flags], cwd, case+'-reference')
                validate(reference,case,cwd)
                reference_info = dict(sha256=PERF.digest(reference), bytes=reference.stat().st_size)
                metadata['cases'][case] = dict(source=str(source), reference=reference_info)
                for round_number in range(args.rounds):
                    order = ['clock','allocation'] if round_number % 2 == 0 else ['allocation','clock']
                    for observer in order:
                        cwd = work / observer / case / str(round_number); cwd.mkdir(parents=True)
                        compiler = clock_compiler if observer == 'clock' else allocation_compiler
                        for state in ('cold','warm'):
                            image = cwd / 'image'
                            if image.exists():
                                image.unlink()
                            command = [compiler,'build',source,'-o',image,'--incremental',*flags]
                            if observer == 'clock':
                                command += ['--profile-compiler','--timings=json']
                            result = run(command,cwd,f'{case}-{round_number}-{observer}-{state}')
                            actual = dict(sha256=PERF.digest(image), bytes=image.stat().st_size)
                            assert actual == reference_info, (case, observer, state, 'image changed across observers/cache/reference')
                            if observer == 'clock':
                                parsed = PROFILE.compiler_timing(result.stderr,target)
                                detail = CACHE.cache_detail(parsed)
                                if state == 'cold':
                                    assert parsed['functions']['reused'] == 0
                                else:
                                    assert parsed['functions']['reused'] > 0
                                record = dict(timing=parsed, cache_detail=detail)
                            else:
                                record = dict(runtime_profile=PROFILE.allocation_profile(result.stderr))
                            metadata['observations'].append(dict(case=case, observer=observer, state=state,
                                                                 round=round_number, image=actual, **record))
                            validate(image,case,cwd)
                    checkpoint()
                print('completed cache attribution/' + case, flush=True)
        assert PERF.source_hashes() == sources_before, 'source changed during attribution'
        assert all(PERF.digest(ROOT/path) == digest for path,digest in hashes.items()), 'source/tool changed during attribution'
        assert PERF.digest(builder) == metadata['builder_sha256'], 'builder changed during attribution'
        assert all(PERF.digest(Path(data['path'])) == data['sha256'] for data in metadata['observers'].values()), 'observer compiler changed'
        summary = []
        for case in metadata['cases']:
            for state in ('cold','warm'):
                clock = [x for x in metadata['observations'] if (x['case'],x['state'],x['observer']) == (case,state,'clock')]
                heap = [x['runtime_profile']['counters'] for x in metadata['observations'] if (x['case'],x['state'],x['observer']) == (case,state,'allocation')]
                assert len(clock) == len(heap) == args.rounds
                parent = statistics.median(x['timing']['compiler_detail']['phases']['fragment_cache'] for x in clock)
                for stage in CACHE.STAGES:
                    summary.append(dict(case=case,state=state,observer='clock',metric=stage,unit='ns',samples=len(clock),
                                        median=statistics.median(x['cache_detail']['phases'][stage] for x in clock),
                                        parent_fragment_cache_median=parent))
                for counter in PROFILE.DOGFOOD.COUNTERS:
                    summary.append(dict(case=case,state=state,observer='allocation',metric=counter,unit='events_or_bytes',samples=len(heap),
                                        median=statistics.median_low(x[counter] for x in heap),parent_fragment_cache_median=''))
        PERF.save_csv(report/'summary.csv',summary)
        metadata['complete'] = True; checkpoint()
        (report/'README.md').write_text('''# Fragment-cache attribution

This report separates cache-stage clocks from allocation traffic. The two
observer compilers were built from the same frozen selfhost source by the
recorded normal compiler, with explicit inline-off observer-building policy.
Workload flags are recorded separately. No process wall-clock series is taken.

Each round starts separate empty cache directories for each observer, then
compiles cold and warm. Cold means empty fragment cache, warm OS filesystem.
Only the clock observer uses --profile-compiler/--timings=json. Only the heap
observer compiler is built with --runtime-profile; it receives no timing flags.
Counter traffic includes null/immortal retain/release helper entries and is
neither live nor peak memory. Counts are not combined with clock observations.

Cache-detail stages overlap the parent fragment_cache interval, and are not
added to the eleven-phase covered sum. Durations require valid monotonic clocks;
unavailable values fail validation. Summary clocks are medians; heap counters
use median_low. Work records input byte spans (decoded text length for reads),
parsed-record spans for validation and actual appended words for commit;
failed calls may have no input bytes. Every image matches the normal-compiler nonincremental reference,
and every image's generated/selfhost-hello/Tabell result is checked outside the
observer process. Raw stdout/stderr, commands, source and compiler hashes remain
in the report. Actual Linux execution requires running this tool on Linux.
''')
    except Exception as error:
        metadata['error'] = repr(error); checkpoint(); raise


if __name__ == '__main__':
    main()
