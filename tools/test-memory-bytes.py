#!/usr/bin/env python3
"""Verify public memory/byte signatures, compiler-name hygiene and target traps."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import shutil
import struct
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/memory_bytes'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
MODES = (('mir', ('--mir=on',)),
         ('direct', ('--mir=off', '--inline-opt=off', '--scalar-opt=off',
                     '--ownership-opt=off')))
RANGES = (
    ('negative-start', 'sys.bytes_extend(&mut output, &source, -1, 0)', 103),
    ('negative-end', 'sys.bytes_extend(&mut output, &source, 0, -1)', 103),
    ('reversed', 'sys.bytes_extend(&mut output, &source, 3, 2)', 103),
    ('past-end', 'sys.bytes_extend(&mut output, &source, 0, 5)', 103),
    ('empty-past-end', 'sys.bytes_extend(&mut output, &source, 5, 5)', 103),
    ('extreme-range', 'sys.bytes_extend(&mut output, &source, -2147483648, 2147483647)', 103),
    ('checked-range', 'bytes.extend_range(&mut output, &source, -1, 4)', 104),
)
WASM_RUNNER = '''
const fs = require('node:fs');
const module_ = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
if (WebAssembly.Module.imports(module_).length !== 0) {
    throw new Error('memory-only image acquired host imports');
}
const entry = new WebAssembly.Instance(module_, {}).exports.main;
const expectTrap = process.argv[3] === 'trap';
try {
    const status = entry();
    if (expectTrap) throw new Error('invalid range returned normally: ' + status);
    if (status !== 0) throw new Error('program returned ' + status);
} catch (error) {
    if (!(expectTrap && error instanceof WebAssembly.RuntimeError)) throw error;
}
'''


def host_target():
    system, machine = platform.system(), platform.machine()
    if (system, machine) == ('Darwin', 'arm64'):
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if system == 'Linux' and machine in ('x86_64', 'amd64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host {system}/{machine}')


def invoke(command, work, status=0):
    result = subprocess.run(list(map(str, command)), cwd=work,
                            capture_output=True, text=True, timeout=90)
    assert result.returncode == status, (command, result.returncode,
                                         result.stdout, result.stderr)
    return result


def inspect_image(image, target):
    data = image.read_bytes()
    if target == 'arm64-macos':
        assert data[:4] == b'\xcf\xfa\xed\xfe', image
    elif target == 'wasm32':
        assert data[:8] == b'\0asm\x01\0\0\0', image
    else:
        assert data[:6] == b'\x7fELF\x02\x01', image
        machine = struct.unpack_from('<H', data, 18)[0]
        assert machine == (183 if target == 'arm64-linux' else 62), image
        phoff = struct.unpack_from('<Q', data, 32)[0]
        size, count = struct.unpack_from('<HH', data, 54)
        kinds = [struct.unpack_from('<I', data, phoff + i * size)[0]
                 for i in range(count)]
        assert 3 not in kinds and 2 not in kinds, (image, 'memory-only ELF acquired dynamic dependencies')
    return hashlib.sha256(data).hexdigest()


def program(body, imports='', unsafe=True):
    return ('capability unsafe\n' + imports + '\nfn main() {\n' +
            ('unsafe { ' + body + ' }' if unsafe else body) + '\n}\n')


def diagnostics():
    cases = []
    for operation in ('copy', 'equal'):
        function = 'sys.memory_' + operation
        for suffix, arguments, message in (
            ('few', 'ptr(0), ptr(0)', 'wrong argument count'),
            ('many', 'ptr(0), ptr(0), usize(0), ptr(0)', 'too many arguments'),
            ('lhs-type', 'i64(0), ptr(0), usize(0)', 'type mismatch: ptr / i64'),
            ('rhs-type', 'ptr(0), usize(0), usize(0)', 'type mismatch: ptr / usize'),
            ('count-type', 'ptr(0), ptr(0), u64(0)', 'type mismatch: usize / u64'),
        ):
            cases.append((operation + '-' + suffix,
                          program(function + '(' + arguments + ')', 'import std.sys as sys'), message))
        cases.append((operation + '-unsafe',
                      program(function + '(ptr(0), ptr(0), usize(0))',
                              'import std.sys as sys', unsafe=False), 'requires unsafe'))
        cases.append(('bytes-' + operation + '-unsafe',
                      program('bytes.' + operation + '_memory(ptr(0), ptr(0), usize(0))',
                              'import std.bytes as bytes', unsafe=False), 'requires unsafe'))
    prefix = 'mut output: []u8 = [0]; input: []u8 = [1]; '
    for name, body, message in (
        ('extend-few', 'sys.bytes_extend(&mut output, &input, 0)', 'wrong argument count'),
        ('extend-many', 'sys.bytes_extend(&mut output, &input, 0, 1, 1)', 'too many arguments'),
        ('extend-output', 'mut wrong: []i32 = [0]; sys.bytes_extend(&mut wrong, &input, 0, 1)', 'type mismatch'),
        ('extend-input', 'wrong: []i32 = [0]; sys.bytes_extend(&mut output, &wrong, 0, 1)', 'type mismatch'),
        ('extend-mutable', 'sys.bytes_extend(&output, &input, 0, 1)', 'mutable parameter'),
        ('extend-start', 'sys.bytes_extend(&mut output, &input, i64(0), 1)', 'type mismatch: i32 / i64'),
        ('extend-end', 'sys.bytes_extend(&mut output, &input, 0, usize(1))', 'type mismatch: i32 / usize'),
        ('append-few', 'sys.bytes_append_text(&mut output)', 'wrong argument count'),
        ('append-many', 'sys.bytes_append_text(&mut output, "", 0)', 'too many arguments'),
        ('append-output', 'mut wrong: []i32 = [0]; sys.bytes_append_text(&mut wrong, "")', 'type mismatch'),
        ('append-text', 'sys.bytes_append_text(&mut output, &input)', 'type mismatch'),
        ('append-mutable', 'sys.bytes_append_text(&output, "")', 'mutable parameter'),
    ):
        cases.append((name, program(prefix + body, 'import std.sys as sys', unsafe=False), message))
    # Intrinsics remain private to the real bundled sys module, even in unsafe code.
    for name, body in (
        ('memory-copy', '__sys_memory_copy(ptr(0), ptr(0), usize(0))'),
        ('memory-equal', '__sys_memory_equal(ptr(0), ptr(0), usize(0))'),
        ('bytes-extend', prefix + '__sys_bytes_extend(&mut output, &input, 0, 1)'),
        ('bytes-append', prefix + '__sys_bytes_append_text(&mut output, "")'),
    ):
        cases.append(('reserved-' + name, program(body), 'reserved compiler identifier'))
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a native target through this command')
    parser.add_argument('--node', default='node', help='Node executable for real Wasm execution')
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    node = shutil.which(args.node)
    if node is None:
        parser.error('Node is required to execute the Wasm memory contracts')
    host = host_target()
    runners = {host: []}
    for value in args.runner:
        target, separator, command = value.partition('=')
        if not separator or target not in TARGETS[:-1] or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    checks = images = rejections = native_runs = wasm_runs = static_elf = 0
    image_hashes = []
    sources = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
               for path in sorted(FIXTURES.iterdir()) if path.is_file()}
    with tempfile.TemporaryDirectory(prefix='dunder-memory-bytes-') as temporary:
        work = Path(temporary).resolve()
        wasm_runner = work / 'run-wasm.cjs'; wasm_runner.write_text(WASM_RUNNER)
        valid = []
        for name in ('memory', 'append'):
            # Copy source alone: imports must resolve through the bundled public API.
            source = work / (name + '.ddr')
            source.write_bytes((FIXTURES / source.name).read_bytes())
            valid.append((name, source, 0))
        template = (FIXTURES / 'range.ddr.in').read_text()
        for name, call, status in RANGES:
            source = work / (name + '.ddr')
            source.write_text(template.replace('@CALL@', call))
            valid.append((name, source, status))
        for name, source, status in valid:
            for target in TARGETS:
                for mode, flags in MODES:
                    checked = invoke([compiler, 'check', source, '--agent', '--target', target, *flags], work)
                    assert json.loads(checked.stdout.splitlines()[-1])['kind'] == 'ok', checked
                    checks += 1
                    image = work / f'{name}-{target}-{mode}'
                    invoke([compiler, 'build', source, '--target', target, '--jobs', '1',
                            *flags, '-o', image], work)
                    digest = inspect_image(image, target); images += 1
                    image_hashes.append({'fixture': name, 'target': target, 'mode': mode, 'sha256': digest})
                    if target.endswith('-linux'):
                        static_elf += 1
                    if target == 'wasm32':
                        result = invoke([node, wasm_runner, image, 'trap' if status else 'ok'], work)
                        assert not result.stdout and not result.stderr, result
                        wasm_runs += 1
                    elif target in runners:
                        result = invoke([*runners[target], image], work, status)
                        if status == 0:
                            assert not result.stdout and not result.stderr, result
                        native_runs += 1
        for name, source_text, message in diagnostics():
            source = work / f'reject-{name}.ddr'; source.write_text(source_text)
            for target in TARGETS:
                for mode, flags in MODES:
                    checked = invoke([compiler, 'check', source, '--agent', '--target', target, *flags], work, 1)
                    report = json.loads(checked.stdout.splitlines()[-1])
                    assert report['kind'] == 'error' and message in report['message'].lower(), (name, target, mode, report)
                    image = work / 'rejected-image'; image.write_bytes(b'previous image')
                    built = invoke([compiler, 'build', source, '--target', target, '--jobs', '1',
                                    *flags, '-o', image], work, 1)
                    assert message in built.stderr.lower(), (name, target, mode, built)
                    assert image.read_bytes() == b'previous image', (name, 'failed build replaced output')
                    rejections += 1
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps({
            'schema': 'dunder-memory-bytes-verification/v1', 'passed': True,
            'host_target': host, 'native_execution_targets': sorted(runners),
            'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(),
            'sources': sources, 'checks': checks, 'images': images,
            'native_executions': native_runs, 'wasm_executions': wasm_runs,
            'static_elf_images': static_elf, 'diagnostic_contracts': rejections,
            'image_hashes': image_hashes}, indent=2) + '\n')
    print(f'Memory bytes: {checks} checks, {images} images ({static_elf} static ELF), '
          f'{native_runs} native executions ({", ".join(runners)}), {wasm_runs} Wasm executions, '
          f'{rejections} check/build diagnostic contracts passed')


if __name__ == '__main__':
    main()
