#!/usr/bin/env python3
"""Self-apply an SSA-built compiler, then exercise its real language/runtime paths.

This is correctness dogfood, not a benchmark. Full images, source manifests,
commands and child output survive success and failure. Existing suites run
unchanged; instrumentation only retains their temporary files and process logs.
Linux images are compared/built, never reported as executed on macOS.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tempfile
import traceback

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest():
    files = sorted(p for directory in ('selfhost', 'stdlib')
                   for p in (ROOT / directory).rglob('*') if p.is_file())
    return {str(p.relative_to(ROOT)): digest(p) for p in files}


def append(path, data):
    with path.open('a') as stream:
        stream.write(json.dumps(data, default=str) + '\n')


def retain_tool(arguments):
    """Run one existing suite with isolated, persistent temporary storage."""
    output = Path(arguments[0]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    temporary = output / 'temporary'
    temporary.mkdir()
    tempfile.tempdir = str(temporary)
    original_directory = tempfile.TemporaryDirectory

    class RetainedDirectory(original_directory):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            self._finalizer.detach()
            append(output / 'temporary.jsonl', {'directory': self.name})

        def cleanup(self):
            pass

    tempfile.TemporaryDirectory = RetainedDirectory
    original_popen = subprocess.Popen
    serial = 0

    class LoggedPopen(original_popen):
        def __init__(self, command, *args, **kwargs):
            nonlocal serial
            serial += 1
            self.audit_id = serial
            self.audit_streams = (kwargs.get('stdout'), kwargs.get('stderr'))
            self.audit_captured = (None, None)
            append(output / 'commands.jsonl', {
                'id': serial, 'event': 'start', 'command': command,
                'cwd': kwargs.get('cwd', os.getcwd()),
                'env': kwargs.get('env'),
            })
            try:
                super().__init__(command, *args, **kwargs)
            except BaseException as error:
                append(output / 'commands.jsonl', {
                    'id': self.audit_id, 'event': 'launch_error', 'error': repr(error)})
                raise

        def communicate(self, *args, **kwargs):
            result = super().communicate(*args, **kwargs)
            self.audit_captured = result
            return result

        def __exit__(self, *args):
            try:
                return super().__exit__(*args)
            finally:
                paths = []
                for name, captured, stream in zip(
                        ('stdout', 'stderr'), self.audit_captured, self.audit_streams):
                    if captured is None and hasattr(stream, 'fileno'):
                        # pread leaves the existing suite's stream offset intact.
                        captured = os.pread(stream.fileno(), os.fstat(stream.fileno()).st_size, 0)
                    if captured is not None:
                        path = output / f'command-{self.audit_id:05d}.{name}'
                        path.write_bytes(captured.encode() if isinstance(captured, str) else captured)
                        paths.append(str(path))
                append(output / 'commands.jsonl', {
                    'id': self.audit_id, 'event': 'finish', 'status': self.returncode,
                    'outputs': paths})

    subprocess.Popen = LoggedPopen
    sys.argv = arguments[1:]
    runpy.run_path(sys.argv[0], run_name='__main__')


class Probe:
    def __init__(self, output):
        self.output = output
        self.commands = 0
        self.checks = []
        self.suites = []
        self.images = {}

    def run(self, name, command, timeout=600):
        self.commands += 1
        command = list(map(str, command))
        append(self.output / 'commands.jsonl', {
            'id': self.commands, 'name': name, 'event': 'start',
            'command': command, 'cwd': str(ROOT)})
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=timeout)
        except BaseException as error:
            append(self.output / 'commands.jsonl', {
                'id': self.commands, 'event': 'error', 'error': repr(error)})
            raise
        (self.output / f'{name}.stdout').write_bytes(result.stdout)
        (self.output / f'{name}.stderr').write_bytes(result.stderr)
        append(self.output / 'commands.jsonl', {
            'id': self.commands, 'event': 'finish', 'status': result.returncode,
            'stdout': f'{name}.stdout', 'stderr': f'{name}.stderr'})
        if result.returncode:
            raise AssertionError((name, result.returncode, result.stdout.decode(errors='replace'),
                                  result.stderr.decode(errors='replace')))
        print('PASS', name, flush=True)
        return result

    def build(self, name, compiler, source, mode, jobs=None):
        image = self.output / name
        command = [compiler, 'build', ROOT / 'selfhost' / source,
                   '--mir-ssa=' + mode, '--timings=json', '-o', image]
        if jobs is not None:
            command += ['--jobs=' + str(jobs)]
        result = self.run(name, command)
        report = json.loads(result.stderr)
        mir = report['mir']
        assert mir['ssa_enabled'] is (mode == 'on'), (name, mir)
        if mode == 'on':
            for field in ('ssa_optimized', 'ssa_values', 'ssa_registers', 'ssa_removed_loads'):
                assert mir[field] > 0, (name, field, mir)
        counts = {key: value for key, value in mir.items()
                  if key.startswith('ssa_') and type(value) in (bool, int)}
        self.images[name] = {'sha256': digest(image), 'bytes': image.stat().st_size,
                             'mode': mode, 'jobs': jobs, 'ssa': counts}
        return image

    def same(self, name, actual, expected):
        same = actual.read_bytes() == expected.read_bytes()
        check = {'name': name, 'actual': str(actual), 'expected': str(expected),
                 'actual_sha256': digest(actual), 'expected_sha256': digest(expected),
                 'byte_identical': same}
        self.checks.append(check)
        append(self.output / 'identity-checks.jsonl', check)
        assert same, check
        print('PASS', name, flush=True)

    def suite(self, name, tool, arguments):
        directory = self.output / 'suites' / name
        self.run('suite-' + name, [sys.executable, __file__, '--retained-tool', directory,
                                  ROOT / 'tools' / tool, *arguments], timeout=1800)
        self.suites.append(name)


def main():
    if len(sys.argv) > 1 and sys.argv[1] == '--retained-tool':
        retain_tool(sys.argv[2:])
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, help='frozen standard parallel compiler')
    parser.add_argument('--serial', required=True, type=Path, help='matching official serial image')
    parser.add_argument('--output', required=True, type=Path, help='new persistent artifact directory')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    probe = Probe(output)
    before = manifest()
    (output / 'source-before.json').write_text(json.dumps(before, indent=2) + '\n')
    source_identity = hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest()
    official = output / 'official-parallel'
    serial = output / 'official-serial'
    shutil.copy2(args.compiler.resolve(strict=True), official)
    shutil.copy2(args.serial.resolve(strict=True), serial)
    summary = {'result': 'failed', 'source_manifest_sha256': source_identity,
               'harness_sha256': digest(Path(__file__)),
               'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'official_parallel_sha256': digest(official), 'official_serial_sha256': digest(serial),
               'images': probe.images, 'identity_checks': probe.checks, 'suites': probe.suites,
               'coverage': 'ARM64/macOS and Wasm execution; Linux image-only; no timing assertions'}
    try:
        # Confirm the frozen compiler and the source tree are the same build family.
        baseline = probe.build('baseline-serial-off', official, 'main.ddr', 'off')
        probe.same('official-source-serial', baseline, serial)
        sopt = probe.build('Sopt', official, 'main.ddr', 'on')
        assert sopt.read_bytes() != serial.read_bytes(), 'SSA producer unexpectedly unchanged'
        probe.same('Sopt-serial-off', probe.build('Sopt-serial-off', sopt, 'main.ddr', 'off'), serial)
        probe.same('Sopt-serial-on', probe.build('Sopt-serial-on', sopt, 'main.ddr', 'on'), sopt)
        probe.same('Sopt-parallel-off', probe.build('Sopt-parallel-off', sopt, 'main_parallel.ddr', 'off'), official)
        popt = probe.build('Popt', sopt, 'main_parallel.ddr', 'on')
        reference = probe.build('official-parallel-on', official, 'main_parallel.ddr', 'on')
        probe.same('Popt-independent-producer', popt, reference)
        for jobs in (1, 4):
            for source, short, expected_off, expected_on in (
                    ('main_parallel.ddr', 'parallel', official, popt),
                    ('main.ddr', 'serial', serial, sopt)):
                for mode, expected in (('off', expected_off), ('on', expected_on)):
                    name = f'Popt-jobs{jobs}-{short}-{mode}'
                    probe.same(name, probe.build(name, popt, source, mode, jobs), expected)

        # These suites are intentionally the repository's independent existing
        # consumers, not newly copied assertions about SSA implementation details.
        for name, tool, arguments in (
                ('check', 'test-check.py', [sopt, '--baseline', serial]),
                ('diagnostics', 'test-diagnostics.py', [sopt]),
                ('ownership', 'test-ownership-leases.py', [sopt, '--baseline', serial]),
                # Its optional baseline means pre-record-take compiler and
                # asserts a strict resource improvement, not byte equivalence.
                ('record-take', 'test-record-take-source.py', [sopt]),
                ('tasks', 'test-task-source.py', [sopt, '--source-root', ROOT,
                                               '--output', output / 'task-programs']),
                ('ffi', 'test-ffi.py', [sopt, '--cc', '/usr/bin/clang']),
                ('parallel', 'test-parallel-compiler.py', [popt, '--baseline', sopt,
                                                         '--output', output / 'parallel-programs']),
                ('ssa-source', 'test-ssa-source.py', [popt])):
            probe.suite(name, tool, arguments)
        for target in ('arm64-macos', 'wasm32'):
            cases = ('array_snapshots', 'nested_ownership', 'borrows', 'region_borrows',
                     'expression_order', 'ieee754', 'integer_and_float', 'utf8',
                     'map_set_sort', 'i64_overflow', 'bad_index', 'zero_divisor')
            flags = [part for case in cases for part in ('--case', case)]
            probe.suite('contracts-' + target, 'runtime-contracts.py',
                        [popt, '--target', target, *flags])
        summary['result'] = 'passed'
    except BaseException as error:
        summary['error'] = repr(error)
        (output / 'failure.txt').write_text(traceback.format_exc())
        raise
    finally:
        after = manifest()
        (output / 'source-after.json').write_text(json.dumps(after, indent=2) + '\n')
        summary['source_unchanged'] = before == after
        summary['commands'] = probe.commands
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        if before != after:
            raise AssertionError('compiler or stdlib sources changed during producer QA')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
