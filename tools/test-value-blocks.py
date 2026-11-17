#!/usr/bin/env python3
"""Verify statement scopes, result ownership and exits in if/match value blocks."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile


FIXTURES = Path(__file__).resolve().parents[1] / 'tests/fixtures/value_blocks'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
MODES = ((), ('--mir=off', '--scalar-opt=off', '--ownership-opt=off', '--inline-opt=off'))
OUTPUTS = {
    'values': 'branch values ok',
    'ownership': 'branch ownership ok',
    'exits': 'branch exits ok',
    'interrupted': 'interrupted expressions ok',
}
PROFILED = {'ownership', 'interrupted'}


def host_target():
    system, machine = platform.system(), platform.machine()
    if (system, machine) == ('Darwin', 'arm64'):
        return 'arm64-macos'
    if system == 'Linux' and machine in ('aarch64', 'arm64'):
        return 'arm64-linux'
    if (system, machine) == ('Linux', 'x86_64'):
        return 'x86_64-linux'
    raise RuntimeError(f'unsupported native host {system}/{machine}')


def run(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work, capture_output=True,
                            text=True, timeout=90)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def check_runtime(result, name):
    assert result.stdout == OUTPUTS[name] + '\n', (name, result)
    if name in PROFILED:
        report = json.loads(result.stderr)
        assert report['kind'] == 'runtime_profile', report
        counters = report['counters']
        # Both fixtures repeat the same few allocations for 256 iterations.
        # A leaked operand, aggregate or arm-local owner grows the fresh count.
        assert counters['fresh_allocations'] < 64 and counters['reused_allocations'] > 256, (name, counters)
        assert counters['array_clone_calls'] == counters['record_clone_calls'] == 0, (name, counters)
    else:
        assert result.stderr == '', (name, result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='execute a cross-compiled target through a local wrapper')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    runners = {host_target(): []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)

    validated = built = executed = rejected = 0
    invalid = {}
    for source in sorted(FIXTURES.glob('invalid-*.ddr')):
        header = source.read_text().splitlines()[0]
        assert header.startswith('// error: '), source
        invalid[source] = header.removeprefix('// error: ')
    with tempfile.TemporaryDirectory(prefix='dunder-value-blocks-') as directory:
        work = Path(directory).resolve()
        image = work / 'value-blocks'
        for target in TARGETS:
            for flags in MODES:
                options = ['--target', target, *flags]
                for name in OUTPUTS:
                    source = FIXTURES / (name + '.ddr')
                    checked = run([compiler, 'check', source, '--agent', *options], work)
                    assert json.loads(checked.stdout.splitlines()[-1])['kind'] == 'ok', checked
                    validated += 1
                    profile = ['--runtime-profile'] if name in PROFILED else []
                    run([compiler, 'build', source, *options, *profile, '-o', image], work)
                    built += 1
                    if target in runners:
                        check_runtime(run([*runners[target], image], work), name)
                        executed += 1
                for source, expected in invalid.items():
                    checked = run([compiler, 'check', source, '--agent', *options], work, status=1)
                    diagnostic = json.loads(checked.stdout)
                    assert diagnostic['kind'] == 'error' and expected in diagnostic['message'], (source, diagnostic)
                    assert diagnostic['path'] == str(source), (source, diagnostic)
                    assert 0 <= diagnostic['start'] < diagnostic['end'] <= source.stat().st_size, (source, diagnostic)
                    image.write_bytes(b'previous image')
                    failed = run([compiler, 'build', source, *options, '-o', image], work, status=1)
                    assert diagnostic['message'] in failed.stderr, (source, diagnostic, failed)
                    assert image.read_bytes() == b'previous image', (source, failed)
                    rejected += 1

    print(f'Value blocks passed {validated} checks, {built} native images, {executed} executions '
          f'({", ".join(runners)}), {rejected} check/build rejections with preserved outputs')


if __name__ == '__main__':
    main()
