#!/usr/bin/env python3
"""Verify source assertion diagnostics, evaluation order and native target output."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
MODES = ((), ('--scalar-opt=off', '--ownership-opt=off', '--const-div-opt=off',
              '--frame-free=off', '--range-opt=off', '--mir=off', '--inline-opt=off'))
EXPRESSION = 'self.value == 42 &&\n            /* å 🌍: original spelling */ self.value == 43'
LIBRARY = '''pub struct Probe { value: i32 }
impl Probe {
    pub fn fail(&self) {
        assert(''' + EXPRESSION + ''')
    }
}
'''


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if system == 'Linux' and machine == 'x86_64':
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host {system}/{machine}')


def diagnostic(path, source, expression, marker):
    line = source[:source.index(marker)].count('\n') + 1
    return f'{path}:{line}: assertion failed: {expression}\n'


def image_bytes(image, target):
    data = image.read_bytes()
    if target == TARGETS[0]:
        assert data[:4] == b'\xcf\xfa\xed\xfe', image
    else:
        assert data[:6] == b'\x7fELF\x02\x01', image
        assert int.from_bytes(data[18:20], 'little') == (183 if target == TARGETS[1] else 62)
    return data


class Probe:
    def __init__(self, compiler, work, runners, workers):
        self.compiler = compiler
        self.work = work
        self.runners = runners
        self.workers = workers
        self.images = self.executions = self.cli_runs = self.test_suites = 0

    def call(self, command, status=0):
        result = subprocess.run(list(map(str, command)), cwd=self.work,
                                capture_output=True, text=True, timeout=90)
        assert result.returncode == status, (command, result.returncode,
                                              result.stdout, result.stderr)
        return result

    def build(self, source, image, target, flags=(), workers=1, incremental=False):
        options = ['--target', target, '--jobs', workers, '--timings=json', *flags]
        if incremental:
            options.append('--incremental')
        result = self.call([self.compiler, 'build', source, *options, '-o', image])
        report = json.loads(result.stderr)
        data = image_bytes(image, target)
        self.images += 1
        return data, report

    def execute(self, image, target, stdout, stderr='', status=0):
        if target not in self.runners:
            return
        result = self.call([*self.runners[target], image], status)
        assert (result.stdout, result.stderr) == (stdout, stderr), (image, result.stdout,
                                                                   result.stderr, stdout, stderr)
        self.executions += 1

    def matrix(self, source, stdout, stderr='', status=0):
        for target in TARGETS:
            for mode, flags in enumerate(MODES):
                images = []
                for workers in self.workers:
                    image = self.work / f'{source.stem}-{target}-{mode}-{workers}'
                    data, _ = self.build(source, image, target, flags, workers)
                    images.append(data)
                    self.execute(image, target, stdout, stderr, status)
                assert all(data == images[0] for data in images), (source, target, mode)

    def cli_run(self, source, stdout, stderr='', status=0):
        for mode, flags in enumerate(MODES):
            for workers in self.workers:
                image = self.work / f'cli-{source.stem}-{mode}-{workers}'
                result = self.call([self.compiler, 'run', source, '--jobs', workers,
                                    *flags, '-o', image], status)
                assert (result.stdout, result.stderr) == (stdout, stderr), result
                self.cli_runs += 1


def ordinary_contracts(probe, library):
    failed = probe.work / 'condition.ddr'
    source = '''fn condition() -> bool { print("condition once"); false }
fn main() {
    assert(condition())
    print("unreachable")
}
'''
    failed.write_text(source)
    error = diagnostic(failed, source, 'condition()', 'assert(')
    probe.matrix(failed, 'condition once\n', error, 104)
    probe.cli_run(failed, 'condition once\n', error, 104)

    passed = probe.work / 'passing.ddr'
    passed.write_text('''fn condition(count: &mut i32) -> bool { count = count + 1; true }
fn main() { mut count := 0; assert(condition(&mut count)); print(count) }
''')
    probe.matrix(passed, '1\n')
    probe.cli_run(passed, '1\n')

    imported = probe.work / 'imported.ddr'
    imported.write_text('''import "./support å" as support
fn main() { probe := support.Probe { value: 42 }; probe.fail(); print("unreachable") }
''')
    error = diagnostic(library, LIBRARY, EXPRESSION, 'assert(')
    probe.matrix(imported, '', error, 104)
    probe.cli_run(imported, '', error, 104)

    interpolated = probe.work / 'interpolated.ddr'
    source = 'fn main(){x:=1;assert("2" == f"{x}")}\n'
    interpolated.write_text(source)
    # Interpolation expands into synthetic tokens; its closing anchor must
    # still include the original quote at the end of the asserted expression.
    error = diagnostic(interpolated, source, '"2" == f"{x}"', 'assert(')
    probe.matrix(interpolated, '', error, 104)


def cache_contracts(probe, library):
    source = probe.work / 'cached.ddr'
    source.write_text('''import "./support å" as support
fn cached() -> i32 { mut n := 0; for i in 0..2 { n = n + i }; n }
fn main() { print(cached()); probe := support.Probe { value: 42 }; probe.fail() }
''')
    for target in TARGETS:
        for mode, flags in enumerate(MODES):
            library.write_text(LIBRARY)
            images, reports = [], []
            for round in range(2):
                image = probe.work / f'cached-{target}-{mode}-{round}'
                data, report = probe.build(source, image, target, flags, incremental=True)
                images.append(data)
                reports.append(report)
                probe.execute(image, target, '1\n',
                              diagnostic(library, LIBRARY, EXPRESSION, 'assert('), 104)
            assert images[0] == images[1], (target, mode, 'warm image changed')
            assert reports[1]['functions']['reused'] > 0, (target, mode, reports)
            # The expression and tokens remain the same. A reused object must
            # never retain a location from before this source-only line shift.
            shifted = '\n\n' + LIBRARY
            library.write_text(shifted)
            image = probe.work / f'shifted-{target}-{mode}'
            probe.build(source, image, target, flags, incremental=True)
            probe.execute(image, target, '1\n',
                          diagnostic(library, shifted, EXPRESSION, 'assert('), 104)
    library.write_text(LIBRARY)


def worker_contracts(probe):
    source = probe.work / 'workers.ddr'
    # String parameters keep these bodies out of the scalar expression inline
    # catalog. Their assertions must carry metadata through general workers.
    bodies = [f'fn worker{i}(text: string) -> i32 {{ assert(text.len == 2); text.len + {i} }}'
              for i in range(16)]
    main = ('fn main() {\n    print(' + ' + '.join(f'worker{i}("ok")' for i in range(16)) +
            ');\n    _ := worker7("");\n    print("unreachable")\n}\n')
    text = '\n'.join(bodies) + '\n' + main
    source.write_text(text)
    marker = 'assert(text.len == 2)'
    begin = text.index('fn worker7(')
    line = text[:text.index(marker, begin)].count('\n') + 1
    error = f'{source}:{line}: assertion failed: text.len == 2\n'
    for target in TARGETS:
        for mode, flags in enumerate(MODES):
            images = []
            for workers in probe.workers:
                image = probe.work / f'workers-{target}-{mode}-{workers}'
                data, report = probe.build(source, image, target, flags, workers)
                images.append(data)
                probe.execute(image, target, '152\n', error, 104)
                if workers > 1:
                    frontend = report['frontend']
                    assert frontend['general_bodies'] >= 8, frontend
                    assert frontend['workers_used'] > 0 and frontend['peak_total_jobs'] >= 2, frontend
            assert all(data == images[0] for data in images), (target, mode, 'worker image changed')


def test_contracts(probe, library):
    cases = probe.work / 'cases.ddr'
    expression = '(value == 42 &&\n        /* å 🌍: implicit assertion */ value == 43)'
    text = '''import "./support å" as support
@test fn implicit_failure() {
    value := 42
    ''' + expression + '''
}
@test fn explicit_method_failure() {
    probe := support.Probe { value: 42 }
    probe.fail()
}
'''
    cases.write_text(text)
    root = probe.work / 'test_root.ddr'
    root.write_text('import cases\n@test fn survivor() { true }\n')
    implicit = diagnostic(cases, text, expression, expression)
    explicit = diagnostic(library, LIBRARY, EXPRESSION, 'assert(')
    for mode, flags in enumerate(MODES):
        for workers in probe.workers:
            output = probe.work / f'tests-{mode}-{workers}'
            result = probe.call([probe.compiler, 'test', root, '--jobs', workers,
                                 *flags, '-o', output], 1)
            assert f'FAIL {cases}::implicit_failure (exit 104)' in result.stdout, result
            assert f'FAIL {cases}::explicit_method_failure (exit 104)' in result.stdout, result
            assert f'PASS {root}::survivor' in result.stdout, result
            assert result.stdout.endswith('1 passed, 2 failed\n'), result
            # Child stderr is inherited, so parallel groups may finish in
            # either order. Each complete diagnostic must appear exactly once.
            assert result.stderr in (implicit + explicit, explicit + implicit), result
            probe.test_suites += 1


def api_contracts(probe):
    api = probe.work / 'compiler-api'
    fixture = ROOT / 'tests/fixtures/interpolation/api.ddr'
    built = probe.call([probe.compiler, 'build', fixture, '--jobs', '1', '-o', api])
    assert built.stderr == '', built
    source = probe.work / 'api-input.ddr'
    source.write_text('\nfn main(){\n assert(1==2)\n}\n')
    for number, target in enumerate(TARGETS):
        image = probe.work / f'api-{target}'
        result = probe.call([api, source, number, 'direct', image])
        assert (result.stdout, result.stderr) == ('api ok\n', ''), result
        image_bytes(image, target)
        probe.images += 1
        # The API receives source text, not a filename. Tokens must retain
        # its real lines and expression without inventing a disk path.
        probe.execute(image, target, '', '<source>:3: assertion failed: 1==2\n', 104)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a target through an emulator or wrapper; repeat per target')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    # Only the ARM64/macOS compiler entry currently provides compiler workers.
    workers = (1, 4) if host == TARGETS[0] else (1,)
    with tempfile.TemporaryDirectory(prefix='dunder-assert-diagnostics-') as temporary:
        work = Path(temporary).resolve()
        library = work / 'support å.ddr'
        library.write_text(LIBRARY)
        probe = Probe(compiler, work, runners, workers)
        ordinary_contracts(probe, library)
        cache_contracts(probe, library)
        worker_contracts(probe)
        test_contracts(probe, library)
        api_contracts(probe)
        print(f'assert diagnostics: {probe.images} native images, {probe.executions} target executions '
              f'({", ".join(runners)}), {probe.cli_runs} CLI runs, {probe.test_suites} test suites; '
              'exact imported locations/expressions, once-only conditions, silent success, '
              'workers, cache line shifts and direct API provenance passed')


if __name__ == '__main__':
    main()
