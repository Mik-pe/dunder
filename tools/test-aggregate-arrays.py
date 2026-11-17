#!/usr/bin/env python3
"""Verify aggregate-array values, ownership and metadata on native and Wasm runtimes."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/aggregate_arrays'
NATIVE = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
TARGETS = (*NATIVE, 'wasm32')
MODES = (('default', ()), ('direct', ('--scalar-opt=off', '--ownership-opt=off', '--mir=off')))
PROGRAMS = {
    'pod': 'pod aggregate arrays ok\n',
    'owned': 'owned aggregate arrays ok\n',
    'enums': 'enum aggregate arrays ok\n',
    'nested': 'nested aggregate arrays ok\n',
    'c_layout': 'c layout aggregate arrays ok\n',
    'recursive': 'recursive aggregate arrays ok\n',
    'resources': 'resource aggregate arrays ok\n',
    'scratch': 'scratch aggregate compatibility ok\n',
    'remove': 'ordered array removal ok\n',
    'inference': 'integer array inference, length names and string owners ok\n',
}
BOUNDS_TRAPS = ('remove-negative', 'remove-past-end', 'remove-empty')
WASM_HOST = r'''
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const moduleImage = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
let instance;
const print = value => process.stdout.write(String(value) + '\n');
const host = {
    print_i32: print,
    print_i64: print,
    print_u32: value => print(value >>> 0),
    print_u64: value => print(BigInt.asUintN(64, value)),
    print_bool: value => print(value === 0 ? 'false' : 'true'),
    print_rune: value => print(String.fromCodePoint(value)),
    print_string: pointer => {
        const memory = instance.exports.memory.buffer;
        const address = pointer >>> 0;
        assert.ok(address + 8 <= memory.byteLength, 'invalid string header');
        const length = new DataView(memory).getBigUint64(address, true);
        assert.ok(length <= BigInt(memory.byteLength - address - 8), 'invalid string bytes');
        const bytes = new Uint8Array(memory, address + 8, Number(length));
        print(new TextDecoder('utf-8', {fatal: true}).decode(bytes));
    },
};
for (const item of WebAssembly.Module.imports(moduleImage)) {
    assert.equal(item.kind, 'function', JSON.stringify(item));
    assert.equal(item.module, 'dunder', JSON.stringify(item));
    assert.equal(typeof host[item.name], 'function', 'unexpected host import ' + item.name);
}
instance = new WebAssembly.Instance(moduleImage, {dunder: host});
assert.equal(typeof instance.exports.main, 'function');
if (process.argv[3] === 'bounds') {
    assert.throws(instance.exports.main, WebAssembly.RuntimeError);
} else {
    const result = instance.exports.main();
    assert.ok(result === undefined || result === 0, 'nonzero main result ' + result);
}
'''


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return NATIVE[0]
    if system == 'Linux' and machine in ('aarch64', 'arm64'):
        return NATIVE[1]
    if system == 'Linux' and machine == 'x86_64':
        return NATIVE[2]
    raise RuntimeError(f'unsupported compiler host {system}/{machine}')


def run(command, cwd, *, status=0, output=None):
    result = subprocess.run(list(map(str, command)), cwd=cwd, capture_output=True,
                            text=True, timeout=90)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output and result.stderr == '', (command, result.stdout, result.stderr)
    return result


def image_header(image, target):
    data = image.read_bytes()
    if target == NATIVE[0]:
        assert data[:4] == b'\xcf\xfa\xed\xfe', (image, data[:20])
    elif target in NATIVE[1:]:
        assert data[:6] == b'\x7fELF\x02\x01', (image, data[:20])
        assert int.from_bytes(data[18:20], 'little') == (183 if target == NATIVE[1] else 62)
    else:
        assert data[:8] == b'\x00asm\x01\x00\x00\x00', (image, data[:8])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a native target through an explicit local emulator or wrapper')
    parser.add_argument('--node', default='node', help='Node executable used for actual Wasm execution')
    parser.add_argument('--serial', action='store_true', help='use jobs 1 only with a serial compiler entry')
    parser.add_argument('--case', action='append', default=[], metavar='NAME',
                        help='select a fixture by stem; repeat for focused bring-up')
    parser.add_argument('--skip-cache', action='store_true', help='omit incremental cache comparisons during focused bring-up')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in NATIVE or not shlex.split(command):
            parser.error('--runner requires native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    expected_errors = json.loads((FIXTURES.parent / 'diagnostics.json').read_text())
    invalid = sorted(FIXTURES.glob('invalid-*.ddr'))
    known = set(PROGRAMS).union(BOUNDS_TRAPS, (path.stem for path in invalid))
    if set(args.case) - known:
        parser.error('unknown fixture: ' + ', '.join(sorted(set(args.case) - known)))
    selected = set(args.case) or known
    programs = [name for name in (*PROGRAMS, *BOUNDS_TRAPS) if name in selected]
    invalid = [path for path in invalid if path.stem in selected]
    if programs and shutil.which(args.node) is None:
        parser.error('Node is required for the Wasm runtime matrix; provide --node PATH')
    images = native_runs = wasm_runs = checks = failed_publications = comparisons = 0

    def jobs_for(target):
        return (1, 4) if host == target == NATIVE[0] and not args.serial else (1,)

    with tempfile.TemporaryDirectory(prefix='dunder-aggregate-arrays-') as temporary:
        work = Path(temporary).resolve()
        wasm_host = work / 'wasm-host.cjs'
        wasm_host.write_text(WASM_HOST)

        def execute(image, target, output, *, bounds_trap=False):
            nonlocal native_runs, wasm_runs
            image_header(image, target)
            if target == 'wasm32':
                run([args.node, wasm_host, image, 'bounds' if bounds_trap else 'ok'], work, output=output)
                wasm_runs += 1
            elif target in runners:
                run([*runners[target], image], work, status=103 if bounds_trap else 0, output=output)
                native_runs += 1

        for name in programs:
            source = FIXTURES / (name + '.ddr')
            for target in TARGETS:
                for mode, flags in MODES:
                    for jobs in jobs_for(target):
                        image = work / f'{name}-{target}-{mode}-{jobs}'
                        run([compiler, 'build', source, '--target', target, '--jobs', jobs,
                             *flags, '-o', image], ROOT)
                        images += 1
                        execute(image, target, PROGRAMS.get(name, ''), bounds_trap=name in BOUNDS_TRAPS)
            print('PASS aggregate arrays', name, 'native3 + executed Wasm, default/direct MIR', flush=True)

        for source in invalid:
            message = expected_errors[source.relative_to(FIXTURES.parent).as_posix()]
            assert message, source
            diagnostics = []
            for target in TARGETS:
                for mode, flags in MODES:
                    for jobs in jobs_for(target):
                        options = ['--target', target, '--jobs', jobs, *flags]
                        checked = run([compiler, 'check', source, '--agent', *options], ROOT, status=1)
                        record = json.loads(checked.stdout)
                        assert record['kind'] == 'error' and record['code'] == 'E1000', (source, record)
                        assert message in record['message'], (source, message, record)
                        assert record['path'] == str(source), (source, record)
                        assert 0 <= record['start'] < record['end'] <= len(source.read_bytes()), record
                        diagnostics.append(record)
                        checks += 1
                        image = work / 'previous-image'
                        image.write_bytes(b'previous valid image')
                        built = run([compiler, 'build', source, *options, '-o', image], ROOT, status=1)
                        assert record['message'] in built.stderr, (source, record, built.stderr)
                        assert image.read_bytes() == b'previous valid image'
                        failed_publications += 1
            assert all(record == diagnostics[0] for record in diagnostics), (source, diagnostics)
            print('PASS aggregate diagnostic', source.stem, flush=True)

        # An owning nested record installs element callbacks in its array
        # metadata. Reused fragments must retain those identities and exactly
        # match a cold image, rather than depending on a previous process's slots.
        if 'owned' in selected and not args.skip_cache:
            project = work / 'cached-project'
            shutil.copytree(FIXTURES, project)
            for target in NATIVE:
                for mode, flags in MODES:
                    cold = work / f'cold-{target}-{mode}'
                    warm = work / f'warm-{target}-{mode}'
                    options = ['--target', target, '--jobs', 1, '--incremental', '--timings=json', *flags]
                    first = run([compiler, 'build', project / 'owned.ddr', *options, '-o', cold], project)
                    second = run([compiler, 'build', project / 'owned.ddr', *options, '-o', warm], project)
                    before, after = json.loads(first.stderr), json.loads(second.stderr)
                    assert before['functions']['reused'] == 0, before
                    assert after['functions']['reused'] > 0, after
                    assert after['functions']['emitted'] < before['functions']['emitted'], (before, after)
                    assert cold.read_bytes() == warm.read_bytes(), (target, mode)
                    images += 2
                    comparisons += 1
                    execute(cold, target, PROGRAMS['owned'])
                    execute(warm, target, PROGRAMS['owned'])
            print('PASS aggregate callbacks in cold/warm incremental images', flush=True)

    print(f'aggregate arrays: {images} native/Wasm images, {native_runs} native executions '
          f'({", ".join(runners)}), {wasm_runs} Wasm executions, {checks} exact diagnostics, '
          f'{failed_publications} preserved failed outputs, {comparisons} cold/warm comparisons; '
          'POD/owning records, payload enums, nested arrays, recursive cloneability and scratch compatibility passed')


if __name__ == '__main__':
    main()
