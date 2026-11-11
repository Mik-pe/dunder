#!/usr/bin/env python3
"""Check, size or measure source task throughput, never compiler parallelism.

One binary per case; capacity1/2/4 is its sole differing runtime input. Samples
include process startup, input setup, value clones, OS thread creation, work,
joins, output and scope teardown. Capacity1 still creates real task threads.
"""
import argparse
import csv
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
COHORT = ROOT / 'benchmarks/tasks'
PROTOCOL = 'dunder-source-task-throughput-v1'
ENV = {'PATH': '/no-external-tools'}
CAPACITIES = (1, 2, 4)
PERMUTATIONS = tuple(itertools.permutations(CAPACITIES))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reference(name, count, length):
    output = []
    original = [(i * 17 + 1729) % 1009 for i in range(length)]
    for job in range(32):
        if name == 'cpu':
            value = 1729 + job * 7919
            checksum = 0
            for _ in range(count):
                value = (value * 1664525 + 1013904223) % 2147483647
                checksum += value
            output += [checksum, value]
        else:
            values = list(original)
            for cycle in range(count):
                values = [(value * 48271 + job + cycle) % 2147483647 for value in values]
            output += [sum(value * (i + 1) for i, value in enumerate(values)),
                       values[0] if values else 0, values[-1] if values else 0]
    if name == 'array':
        output += [sum(value * (i + 1) for i, value in enumerate(original)),
                   original[0] if original else 0, original[-1] if original else 0]
    if any(not -(1 << 63) <= value < 1 << 63 for value in output):
        raise ValueError('independent result exceeds signed64')
    return ''.join(str(value) + '\n' for value in output).encode()


def serial_source(name, work):
    module = os.path.relpath(COHORT / (name + '_worker'), work).replace(os.sep, '/')
    header = 'import ' + json.dumps(module) + ' as kernel\nimport std.sys as sys\nimport std.parse as parse\n'
    if name == 'cpu':
        body = '''fn main(){count:=i32(parse.integer(sys.arg(1)).value)
for job in 0..32{result:=kernel.run(kernel.Request{seed:1729+i64(job)*7919,steps:count});print(result.checksum);print(result.last)}}
'''
    else:
        body = '''fn main(){count:=i32(parse.integer(sys.arg(1)).value);length:=i32(parse.integer(sys.arg(2)).value)
var values:[]i64=[];for i in 0..length{values.push((i64(i)*17+1729)%1009)}
for job in 0..32{result:=kernel.run(kernel.Request{values:values,passes:count,job:i64(job)});print(result.checksum);print(result.first);print(result.last)}
var checksum:i64=0;for i in 0..values.len{checksum=checksum+values[i]*i64(i+1)};print(checksum)
if values.len>0{print(values[0]);print(values[values.len-1])}else{print(0);print(0)}}
'''
    return header + body


def save_csv(path, rows):
    if not rows:
        return
    with path.open('w', newline='') as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader(); writer.writerows(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='mode', required=True)
    for mode in ('check', 'calibrate', 'measure'):
        sub = commands.add_parser(mode)
        sub.add_argument('compiler', type=Path)
        if mode == 'check':
            sub.add_argument('--report', type=Path)
        else:
            sub.add_argument('report', type=Path)
        sub.add_argument('--manifest', type=Path, default=COHORT / 'workloads.json')
        sub.add_argument('--case', action='append', choices=('cpu', 'array'))
        sub.add_argument('--compiler-flag', action='append', default=[])
        if mode == 'check':
            sub.add_argument('--reference-compiler', type=Path,
                             help='also validate the ordinary kernels with an older serial compiler')
        elif mode == 'calibrate':
            sub.add_argument('--target-ms', type=float, default=250)
        else:
            sub.add_argument('--rounds', type=int, default=30)
    args = parser.parse_args()
    if platform.system() != 'Darwin' or platform.machine() != 'arm64':
        parser.error('the current source task implementation requires ARM64 macOS')
    if args.mode == 'measure' and args.rounds < 1:
        parser.error('--rounds must be positive')
    if args.mode == 'calibrate' and (not math.isfinite(args.target_ms) or args.target_ms < 100):
        parser.error('--target-ms must be finite and at least100')
    compiler = args.compiler.resolve(strict=True)
    older = args.reference_compiler.resolve(strict=True) if args.mode == 'check' and args.reference_compiler else None
    manifest_path = args.manifest.resolve(strict=True)
    manifest = json.loads(manifest_path.read_text())
    if (manifest.get('protocol') != PROTOCOL or manifest.get('jobs') != 32 or
            manifest.get('batch_width') != 4 or manifest.get('capacities') != list(CAPACITIES)):
        parser.error('manifest protocol/geometry differs from the frozen source program')
    if args.mode == 'measure' and manifest.get('frozen') is not True:
        parser.error('measurement requires reviewed frozen work counts; calibrate once first')
    report = args.report.resolve() if args.report else None
    if report:
        report.mkdir(parents=True, exist_ok=True)
        if any(report.iterdir()):
            parser.error('report directory must be empty')
    cases = list(dict.fromkeys(args.case or ('cpu', 'array')))
    paths = {Path(__file__).resolve(), manifest_path, compiler, *COHORT.glob('*.ddr')}
    if older:
        paths.add(older)
    before = {str(path): sha(path) for path in paths}
    metadata = dict(protocol=PROTOCOL, mode=args.mode, complete=False, host=platform.uname()._asdict(),
                    compiler=str(compiler), compiler_sha256=sha(compiler), compiler_flags=args.compiler_flag,
                    manifest=manifest, manifest_sha256=sha(manifest_path), inputs_sha256=before,
                    environment=ENV, commands=[], workloads={}, limitations=[
                        'Source language task throughput on ARM64macOS, not parallel compiler speedup',
                        'Capacity1 is one-worker task scheduling, not a direct-call serial baseline',
                        'No C comparison or addition to the four-kernel C acceptance suite',
                        'Whole process wall time includes input/setup/clones/start/join/output/scope teardown',
                        'Thread placement, frequency and OS scheduling are uncontrolled; report host and paired distributions'])
    for key, command in (('revision', ['git', 'rev-parse', 'HEAD']),
                         ('working_tree', ['git', 'status', '--short']),
                         ('hardware', ['/usr/sbin/sysctl', 'machdep.cpu.brand_string', 'hw.memsize', 'hw.ncpu'])):
        metadata[key] = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    if args.mode == 'measure':
        metadata.update(rounds=args.rounds, order='all six capacity permutations rotate; each occurs5 times at30 rounds',
                        warmup='one checked unrecorded fresh process per capacity before each case')
    rows, summaries, calibration = [], [], []
    recommended = json.loads(json.dumps(manifest))
    try:
        with tempfile.TemporaryDirectory(prefix='dunder-task-throughput-') as directory:
            work = Path(directory).resolve()

            def invoke(command, timed=False):
                started = time.perf_counter_ns() if timed else None
                result = subprocess.run([str(x) for x in command], cwd=work, env=ENV,
                                        capture_output=True, timeout=180)
                elapsed = (time.perf_counter_ns() - started) / 1_000_000 if timed else None
                if result.returncode or result.stderr:
                    raise RuntimeError((command, result.returncode, result.stdout, result.stderr))
                metadata['commands'].append(dict(command=[str(x) for x in command], timed=timed,
                                                stdout=result.stdout.decode(), status=result.returncode))
                return elapsed, result.stdout

            for name in cases:
                settings = manifest['cases'][name]
                count = settings['steps' if name == 'cpu' else 'passes']
                length = 0 if name == 'cpu' else settings['length']
                maximum = 8_000_000 if name == 'cpu' else 8192
                if type(count) is not int or not 1 <= count <= maximum or type(length) is not int or not 0 <= length <= 4096:
                    raise ValueError((name, 'work count out of range'))
                if args.mode != 'check' and name == 'array' and length < 1:
                    raise ValueError('measured array work must be nonempty')
                image = work / name
                invoke([compiler, 'build', COHORT / (name + '.ddr'), '-o', image, *args.compiler_flag])
                image_digest = sha(image)
                record = dict(binary_sha256=image_digest, binary_bytes=image.stat().st_size, count=count,
                              length=length, jobs=32, batch_width=4, capacities=list(CAPACITIES))
                metadata['workloads'][name] = record

                def run(capacity, work_count, array_length, expected, timed=False):
                    inputs = [capacity, work_count] + ([array_length] if name == 'array' else [])
                    elapsed, output = invoke([image, *inputs], timed)
                    if output != expected:
                        raise ValueError((name, capacity, work_count, array_length, 'independent result mismatch', output, expected))
                    return elapsed

                if args.mode == 'check':
                    scenarios = [(0, 0), (16, 0)] if name == 'cpu' else [(0, 9), (2, 0), (2, 1), (2, 9)]
                    serial = work / (name + '-serial')
                    if older:
                        source = serial.with_suffix('.ddr'); source.write_text(serial_source(name, work))
                        invoke([older, 'build', source, '-o', serial])
                    for small_count, small_length in scenarios:
                        expected = reference(name, small_count, small_length)
                        for capacity in CAPACITIES:
                            run(capacity, small_count, small_length, expected)
                        if older:
                            _, output = invoke([serial, small_count, *([small_length] if name == 'array' else [])])
                            if output != expected:
                                raise ValueError((name, 'older compiler ordinary kernel mismatch'))
                    record['checked_scenarios'] = scenarios
                    print('PASS', name, 'independent results at capacity1/2/4; empty/snapshot cases', flush=True)
                else:
                    expected = reference(name, count, length)
                    record['expected_stdout'] = expected.decode()
                    for capacity in CAPACITIES:
                        run(capacity, count, length, expected)
                    if args.mode == 'calibrate':
                        minimal = reference(name, 0, length)
                        minimum = run(1, 0, length, minimal, True)
                        elapsed = run(1, count, length, expected, True)
                        if minimum >= args.target_ms * .1:
                            raise ValueError('minimum-work cost exceeds10% of sizing target; choose a larger target before freezing')
                        work_elapsed = elapsed - minimum
                        if work_elapsed <= 0:
                            raise ValueError('sizing observation does not separate work from minimum-work cost')
                        proposed = max(1, min(maximum, math.ceil(count * max(1, args.target_ms - minimum) / work_elapsed)))
                        recommended['cases'][name]['steps' if name == 'cpu' else 'passes'] = proposed
                        calibration.append(dict(case=name, count=count, minimum_work_ms=minimum, elapsed_ms=elapsed,
                                                target_ms=args.target_ms, proposed_count=proposed))
                        print('SIZING', name, 'proposed common count', proposed, '(not a latency distribution)', flush=True)
                    else:
                        for round_number in range(args.rounds):
                            for position, capacity in enumerate(PERMUTATIONS[round_number % len(PERMUTATIONS)]):
                                elapsed = run(capacity, count, length, expected, True)
                                rows.append(dict(case=name, round=round_number, order=position, capacity=capacity,
                                                 elapsed_ms=f'{elapsed:.6f}', binary_sha256=image_digest))
                        for capacity in CAPACITIES:
                            times = sorted(float(row['elapsed_ms']) for row in rows if row['case'] == name and row['capacity'] == capacity)
                            paired = []
                            for round_number in range(args.rounds):
                                pair = {row['capacity']: float(row['elapsed_ms']) for row in rows
                                        if row['case'] == name and row['round'] == round_number}
                                paired.append(pair[1] / pair[capacity])
                            summaries.append(dict(case=name, capacity=capacity, samples=len(times),
                                median_ms=statistics.median(times), p95_ms=times[math.ceil(len(times) * .95) - 1],
                                median_paired_speedup_over_capacity1=statistics.median(paired)))
                        print('COMPLETE', name, args.rounds, 'fresh processes per capacity', flush=True)
                if sha(image) != image_digest:
                    raise ValueError('runtime image changed')
        if any(sha(Path(path)) != digest for path, digest in before.items()):
            raise ValueError('compiler/source/manifest changed during run')
        metadata['complete'] = True
    except Exception as error:
        metadata['error'] = repr(error)
        raise
    finally:
        if report:
            save_csv(report / 'samples.csv', rows)
            save_csv(report / 'summary.csv', summaries)
            save_csv(report / 'calibration.csv', calibration)
            if args.mode == 'calibrate':
                recommended['frozen'] = False
                recommended['sizing'] = dict(compiler_sha256=sha(compiler), observations=calibration,
                                             note='Review these common counts, then set frozen=true before paired measurement.')
                (report / 'recommended-workloads.json').write_text(json.dumps(recommended, indent=2) + '\n')
            (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, sort_keys=True) + '\n')


if __name__ == '__main__':
    main()
