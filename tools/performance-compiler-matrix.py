#!/usr/bin/env python3
"""Fresh-process compiler revision/worker matrix; --validate-only records no timings.

Six frozen cases, warm OS caches, incremental disabled. Revision mode measures
build and (with --include-check) serial check. Worker mode measures builds only,
rotating all six orders of jobs 1/2/4. Validation, image hashing and Wasm execution
are outside measured intervals. No automatic repeats, calibration or filtering.
"""
import argparse
import csv
import json
import math
import os
from pathlib import Path
import platform
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
import performance as perf

ROOT = Path(__file__).resolve().parents[1]
OWNED = '''struct Box{a:[]i64,n:i64}
fn owned(n:i64)->Box{Box{a:[n,n+1,n+2],n:n}}
fn mutate(a:&mut []i64)->i64{var sum:i64=0;for i in 0..a.len{a[i]=a[i]+1;sum=sum+a[i]};sum}
fn relay(n:i64)->Box{owned(n)}
fn main(){x:=relay(5);var a:=x.a;print(mutate(&mut a));print(x.a[0]);print(a[2])}
'''
WASM = r'''const fs=require('node:fs'),assert=require('node:assert/strict');
const print=x=>console.log(x.toString());let instance;
instance=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(process.argv[2])),{dunder:{
print_i32:print,print_i64:print,print_f32:print,print_f64:print,
print_u32:x=>print(x>>>0),print_u64:x=>print(BigInt.asUintN(64,x)),
print_bool:x=>print(Boolean(x)),print_rune:x=>print(String.fromCodePoint(x)),
print_string(p){const b=instance.exports.memory.buffer,n=new DataView(b).getUint32(p,true);
print(new TextDecoder('utf-8',{fatal:true}).decode(new Uint8Array(b,p+8,n)));}}});
const result=instance.exports.main();assert.ok(result===undefined||result===0);
'''
CASES = ('generated', 'selfhost', 'selfhost-parallel', 'tabell', 'hello', 'owned')


def quantile95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


def describe(values):
    return dict(samples=len(values), min=min(values), median=statistics.median(values),
                p95=quantile95(values), max=max(values))


def variants(mode, baseline, candidate):
    if mode == 'revision':
        return {'baseline': (baseline, []), 'candidate': (candidate, [])}
    return {f'jobs{n}': (candidate, [f'--jobs={n}']) for n in (1, 2, 4)}


def summarize(rows, reference):
    result = []
    for operation, case in dict.fromkeys((r['operation'], r['case']) for r in rows):
        selected = [r for r in rows if (r['operation'], r['case']) == (operation, case)]
        base = {r['round']: r for r in selected if r['variant'] == reference}
        for variant in dict.fromkeys(r['variant'] for r in selected):
            group = [r for r in selected if r['variant'] == variant]
            elapsed = [float(r['elapsed_ms']) for r in group]
            paired = [float(r['elapsed_ms']) / float(base[r['round']]['elapsed_ms']) for r in group]
            rss = [int(r['max_rss_bytes']) for r in group]
            result.append(dict(operation=operation, case=case, variant=variant,
                               samples=len(group), median_ms=statistics.median(elapsed),
                               p95_ms=quantile95(elapsed), min_ms=min(elapsed), max_ms=max(elapsed),
                               median_over_reference=statistics.median(elapsed) /
                               statistics.median(float(r['elapsed_ms']) for r in base.values()),
                               paired_ratio_median=statistics.median(paired), paired_ratio_p95=quantile95(paired),
                               median_max_rss_bytes=statistics.median(rss), p95_max_rss_bytes=quantile95(rss),
                               max_rss_bytes=max(rss)))
    return result


class Runner:
    def __init__(self, report, cwd, temporary, dry):
        self.report, self.cwd, self.temporary, self.dry = report, cwd, temporary, dry
        self.count = 0

    def run(self, argv, purpose, measured=False, expected=None):
        assert not (measured and self.dry)
        argv = list(map(str, argv))
        env = dict(PATH='/no-external-tools', TMPDIR=str(self.temporary) + '/')
        # Files avoid pipe deadlock; preparation and output reading are outside the clock.
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
            old_handler = signal.getsignal(signal.SIGALRM)
            old_timer = signal.getitimer(signal.ITIMER_REAL)
            if old_timer != (0.0, 0.0):
                raise RuntimeError('runner requires no preexisting interval timer')
            def timeout(_signum, _frame):
                raise TimeoutError(f'command exceeded 120 seconds: {argv}')
            signal.signal(signal.SIGALRM, timeout)
            process, failure, elapsed, usage = None, None, None, None
            try:
                signal.setitimer(signal.ITIMER_REAL, 120)
                started = time.perf_counter_ns() if measured else None
                process = subprocess.Popen(argv, cwd=self.cwd, env=env, stdout=out, stderr=err,
                                           start_new_session=True)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
                elapsed = (time.perf_counter_ns() - started) / 1_000_000 if measured else None
            except BaseException as error:
                failure = error
                if process is not None and process.returncode is None:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    _, status, _ = os.wait4(process.pid, 0)
                    process.returncode = os.waitstatus_to_exitcode(status)
            finally:
                signal.setitimer(signal.ITIMER_REAL, 0)
                signal.signal(signal.SIGALRM, old_handler)
            out.seek(0); stdout = out.read()
            err.seek(0); stderr = err.read()
        rss = int(usage.ru_maxrss) * (1 if sys.platform == 'darwin' else 1024) if measured and usage else None
        entry = dict(index=self.count, purpose=purpose, argv=argv, cwd=str(self.cwd), environment=env,
                     measured=measured, exit_code=process.returncode if process else None,
                     error=repr(failure) if failure else None, stdout=stdout.decode(errors='replace'),
                     stderr=stderr.decode(errors='replace'), elapsed_ms=elapsed, max_rss_bytes=rss)
        self.count += 1
        with (self.report / 'commands.jsonl').open('a') as log:
            log.write(json.dumps(entry, ensure_ascii=False) + '\n')
        if failure is not None:
            raise failure
        if process.returncode != 0 or (expected is not None and stdout != expected):
            raise RuntimeError(f'command/output failure; see commands.jsonl entry {entry["index"]}: {argv}')
        if measured and (not math.isfinite(elapsed) or elapsed <= 0 or rss <= 0):
            raise RuntimeError('invalid process clock or RSS observation')
        return stdout, elapsed, rss


def sources(root):
    perf.ROOT = root
    result = perf.source_hashes()
    result['examples/hello.ddr'] = perf.digest(root / 'examples/hello.ddr')
    return result


def make_cases(root, work):
    generated, corpus = perf.corpus(work)
    owned = work / 'owned.ddr'; owned.write_text(OWNED)
    with (root / 'projects/tabell/expenses.csv').open(newline='') as stream:
        totals = {}
        for row in csv.DictReader(stream):
            totals[row['team']] = totals.get(row['team'], 0) + int(row['cents'])
    tabell = ('group,total\n' + ''.join(f'{k},{v}\n' for k, v in sorted(totals.items()))).encode()
    if tabell != (root / 'projects/tabell/expected.csv').read_bytes():
        raise RuntimeError('Tabell golden differs from independent CSV arithmetic')
    original = [5, 6, 7]; changed = [v + 1 for v in original]
    owned_expected = f'{sum(changed)}\n{original[0]}\n{changed[-1]}\n'.encode()
    cases = {
        'generated': (generated, [], f'{sum(range(1000)) + 1000 * 96}\n'.encode()),
        'selfhost': (root / 'selfhost/main.ddr', [], b'42\ntrue\n'),
        'selfhost-parallel': (root / 'selfhost/main_parallel.ddr', [], b'42\ntrue\n'),
        'tabell': (root / 'projects/tabell/main.ddr',
                   [root / 'projects/tabell/expenses.csv', 'team', 'cents'], tabell),
        'hello': (root / 'examples/hello.ddr', [], b'42\ntrue\n'),
        'owned': (owned, [], owned_expected),
    }
    return cases, corpus


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('baseline', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('report', type=Path)
    parser.add_argument('--mode', choices=('revision', 'workers'), required=True)
    parser.add_argument('--rounds', type=int, default=30)
    parser.add_argument('--include-check', action='store_true')
    parser.add_argument('--validate-only', action='store_true', help='untimed preflight only; no latency/RSS samples')
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--node', type=Path, default=shutil.which('node'))
    args = parser.parse_args()
    if args.rounds < 1 or (args.mode == 'workers' and args.rounds % 6):
        parser.error('positive rounds required; worker rounds must be divisible by six for balanced order')
    if args.mode == 'workers' and args.include_check:
        parser.error('worker mode is build-only; revision check is the serial negative control')
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        parser.error('this frozen six-case protocol uses ARM64 macOS main/main_parallel entries')
    root = args.source_root.resolve(strict=True)
    baseline, candidate = args.baseline.resolve(strict=True), args.candidate.resolve(strict=True)
    if args.mode == 'workers' and perf.digest(baseline) != perf.digest(candidate):
        parser.error('worker mode requires the identical compiler image for both paths')
    if args.node is None:
        parser.error('Node is required for the independent Wasm consumer gates')
    node = Path(args.node).resolve(strict=True)
    report = args.report.resolve(); report.mkdir(parents=True, exist_ok=True)
    if any(report.iterdir()):
        parser.error('report must be empty; existing measurements are never overwritten')
    drivers = variants(args.mode, baseline, candidate)
    before = sources(root)
    refs = subprocess.check_output(['git', 'show-ref'], cwd=root)
    metadata = dict(complete=False, mode=args.mode, validate_only=args.validate_only, rounds=args.rounds,
                    controller=dict(argv=sys.argv, python=sys.executable, python_version=sys.version),
                    checkout=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
                    working_tree=subprocess.check_output(['git', 'status', '--short'], cwd=root, text=True),
                    host=platform.uname()._asdict(), source_root=str(root), sources_sha256=before,
                    tool_sha256=perf.digest(Path(__file__)), helpers_sha256=perf.digest(Path(perf.__file__)),
                    node=dict(path=str(node), sha256=perf.digest(node)),
                    compiler_sha256={v: perf.digest(p) for v, (p, _) in drivers.items()},
                    compiler_paths={v: str(p) for v, (p, _) in drivers.items()},
                    compiler_flags={v: flags for v, (_, flags) in drivers.items()},
                    scope='whole fresh compiler process; startup/thread clone/start/join/link/publication included',
                    clock='none' if args.validate_only else 'perf_counter_ns immediately before Popen through wait4 exit',
                    rss='none' if args.validate_only else 'wait4.ru_maxrss per child, bytes; includes worker threads; not heap/live bytes',
                    cache='incremental disabled; warm OS filesystem cache; fresh output per sample',
                    order='revision alternates; workers cycle all six permutations',
                    policy='unflagged default SSA policy; no profiling/incremental flags',
                    wasm_scope='generated, hello, owned executed; compiler and file-CLI Wasm hosting not covered',
                    selfhost_scope='main and main_parallel are separate source builds, never whole bootstrap',
                    cases={}, images={}, wasm_preflight={}, runtime_preflight={}, samples_expected=0 if args.validate_only else
                    args.rounds * 6 * len(drivers) * (2 if args.include_check else 1))
    metadata['hardware'] = subprocess.check_output(
        ['/usr/sbin/sysctl', 'machdep.cpu.brand_string', 'hw.memsize'], text=True)
    metadata['rss_raw_unit'] = 'bytes on Darwin; KiB on Linux, normalized to bytes'
    rows = []
    def checkpoint():
        perf.save_csv(report / 'samples.csv', rows)
        (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + '\n')
    with tempfile.TemporaryDirectory(prefix='dunder-compiler-matrix-') as tmp:
        work = Path(tmp); temporary = work / 'tmp'; temporary.mkdir()
        runner = Runner(report, root, temporary, args.validate_only)
        cases, metadata['generated_corpus'] = make_cases(root, work)
        wasm_runner = work / 'wasm.cjs'; wasm_runner.write_text(WASM)
        def validate(case, image):
            _, inputs, expected = cases[case]
            if case.startswith('selfhost'):
                hello = work / 'consumer-hello'
                hello.unlink(missing_ok=True)
                runner.run([image, 'build', root / 'examples/hello.ddr', '-o', hello], 'validate emitted compiler')
                runner.run([hello], 'validate emitted compiler consumer', expected=expected)
            else:
                runner.run([image, *inputs], 'validate native consumer', expected=expected)
        def build(case, variant, purpose, measured=False):
            compiler, flags = drivers[variant]; source, _, _ = cases[case]
            image = work / 'output'
            image.unlink(missing_ok=True)
            _, elapsed, rss = runner.run([compiler, 'build', source, '-o', image, *flags], purpose, measured)
            identity = dict(sha256=perf.digest(image), bytes=image.stat().st_size)
            previous = metadata['images'].setdefault(case, {}).setdefault(variant, identity)
            if identity != previous:
                raise RuntimeError(f'nonreproducible image: {case}/{variant}')
            validate(case, image)
            image.unlink()
            return elapsed, rss, identity
        try:
            for case, (source, inputs, expected) in cases.items():
                metadata['cases'][case] = dict(source=str(source), source_sha256=perf.digest(source),
                                             args=list(map(str, inputs)), expected_stdout=expected.decode())
                for variant in drivers:
                    build(case, variant, 'untimed native preflight')
                    if args.include_check:
                        stdout, _, _ = runner.run([drivers[variant][0], 'check', source, *drivers[variant][1]],
                                                  'untimed check preflight')
                        if not stdout.startswith(f'checked {source} ('.encode()) or not stdout.endswith(b' modules)\n'):
                            raise RuntimeError('invalid check success output')
                if len({i['sha256'] for i in metadata['images'][case].values()}) != 1:
                    raise RuntimeError(f'cross-variant image mismatch in SSA-off scheduling cohort: {case}')
            # Separate, untimed Wasm execution; common print ABI handles bool as well as integers.
            for case in ('generated', 'hello', 'owned'):
                identities = []
                metadata['wasm_preflight'][case] = {}
                for variant, (compiler, flags) in drivers.items():
                    image = work / 'consumer.wasm'
                    image.unlink(missing_ok=True)
                    runner.run([compiler, 'build', cases[case][0], '--target', 'wasm32', '-o', image, *flags],
                               'untimed Wasm preflight')
                    identities.append(perf.digest(image))
                    metadata['wasm_preflight'][case][variant] = dict(sha256=identities[-1], bytes=image.stat().st_size)
                    runner.run([node, wasm_runner, image], 'validate Wasm consumer', expected=cases[case][2])
                if len(set(identities)) != 1:
                    raise RuntimeError(f'cross-variant Wasm image mismatch: {case}')
            # Four established runtime images are built/executed, never benchmarked here.
            for case, inputs, expected in perf.runtime_cases():
                images = {}
                for variant, (compiler, flags) in drivers.items():
                    image = work / 'runtime-image'
                    image.unlink(missing_ok=True)
                    runner.run([compiler, 'build', root / f'benchmarks/runtime/{case}.ddr', '-o', image, *flags],
                               'untimed runtime image preflight')
                    images[variant] = dict(sha256=perf.digest(image), bytes=image.stat().st_size)
                    runner.run([image, *inputs], 'validate independent runtime checksum', expected=f'{expected}\n'.encode())
                metadata['runtime_preflight'][case] = dict(args=inputs, expected_stdout=f'{expected}\n', images=images)
                if len({i['sha256'] for i in images.values()}) != 1:
                    raise RuntimeError(f'runtime images changed; stop for a separately authorized runtime/C protocol: {case}')
            checkpoint()
            if not args.validate_only:
                for case in CASES:
                    for operation in ('build', 'check') if args.include_check else ('build',):
                        for round_number in range(args.rounds):
                            order = perf.order(list(drivers), round_number)
                            for position, variant in enumerate(order):
                                if operation == 'build':
                                    elapsed, rss, image = build(case, variant, 'recorded build', True)
                                else:
                                    compiler, flags = drivers[variant]; source = cases[case][0]
                                    stdout, elapsed, rss = runner.run([compiler, 'check', source, *flags], 'recorded check', True)
                                    if not stdout.startswith(f'checked {source} ('.encode()) or not stdout.endswith(b' modules)\n'):
                                        raise RuntimeError('invalid check success output')
                                    image = dict(sha256='', bytes=0)
                                rows.append(dict(operation=operation, case=case, variant=variant, round=round_number,
                                                 order=position, elapsed_ms=f'{elapsed:.6f}', max_rss_bytes=rss,
                                                 image_sha256=image['sha256'], image_bytes=image['bytes']))
                        checkpoint()
                        print(f'completed {operation}/{case}', flush=True)
            if sources(root) != before or (not args.validate_only and
                                          subprocess.check_output(['git', 'show-ref'], cwd=root) != refs):
                raise RuntimeError('source files or refs changed during run')
            if (subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() != metadata['checkout']
                    or subprocess.check_output(['git', 'status', '--short'], cwd=root, text=True) != metadata['working_tree']):
                raise RuntimeError('source checkout or working-tree state changed during run')
            for variant, (path, _) in drivers.items():
                if perf.digest(path) != metadata['compiler_sha256'][variant]:
                    raise RuntimeError('compiler changed during run')
            if perf.digest(Path(__file__)) != metadata['tool_sha256'] or perf.digest(Path(perf.__file__)) != metadata['helpers_sha256']:
                raise RuntimeError('measurement tool changed during run')
            if perf.digest(node) != metadata['node']['sha256']:
                raise RuntimeError('Node changed during run')
            if len(rows) != metadata['samples_expected']:
                raise RuntimeError('sample count mismatch')
            for case in CASES:
                for operation in ('build', 'check') if args.include_check else ('build',):
                    for variant in drivers:
                        rounds = [r['round'] for r in rows if (r['case'], r['operation'], r['variant']) ==
                                  (case, operation, variant)]
                        if not args.validate_only and sorted(rounds) != list(range(args.rounds)):
                            raise RuntimeError('missing or duplicated rounds')
            reference = 'baseline' if args.mode == 'revision' else 'jobs1'
            summaries = summarize(rows, reference)
            perf.save_csv(report / 'summary.csv', summaries)
            metadata['complete'] = True
            metadata['samples_actual'] = len(rows)
            print(f'PASS {len(cases)} native cases, 3 Wasm cases, 4 runtime image cases; {len(rows)} latency samples')
        finally:
            checkpoint()


if __name__ == '__main__':
    main()
