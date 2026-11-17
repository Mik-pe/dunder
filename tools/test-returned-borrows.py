#!/usr/bin/env python3
"""Verify the lifetime, ownership and native representation of returned shared borrows."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import tempfile


FIXTURES = Path(__file__).resolve().parents[1] / 'tests/fixtures/returned_borrows'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
MODES = ((), ('--mir=off', '--scalar-opt=off', '--ownership-opt=off', '--inline-opt=off'))
ORIGIN = 'returned borrow must come from its declared shared parameter'

# Numeric bounds take precedence over UTF-8 boundaries. Each endpoint runs once.
FAULTS = {
    'order': (103, '1\n2\n'),
    'reversed': (103, ''),
    'past': (103, ''),
    'utf8-start': (106, ''),
    'utf8-end': (106, ''),
    'utf8-empty': (106, ''),
    'bounds-first': (103, ''),
}

INVALID_DECLARATIONS = {
    'local-owner': ('fn bad(src: &i32) -> &i32 { local := src + 1; &local }', ORIGIN),
    'local-alias': ('''fn bad(src: &[]i32) -> &[]i32 {
        local := src.clone()
        a := &local[0..local.len]
        return a
    }''', ORIGIN),
    'local-forward': ('''fn bad(src: &[]i32) -> &[]i32 {
        local := src.clone()
        return views.window(local, 0, 1)
    }''', ORIGIN),
    'temporary-forward': ('''fn bad(src: &[]i32) -> &[]i32 {
        return views.window(src.clone(), 0, 1)
    }''', ORIGIN),
    'region-storage': ('''fn bad(src: &[]i32) -> &[]i32 {
        region scratch(16) {
            mut v: []i32 = scratch.alloc(1)
            return views.window(&v, 0, 1)
        }
        return src
    }''', ORIGIN),
    'raw-reference': ('''unsafe fn bad(src: &i32, p: ptr) -> &i32 { return ref_at(p) }''', ORIGIN),
    'widened-string-byte': ('''fn id(n: &i32) -> &i32 { n }
    fn bad(text: &string) -> &i32 { id(text[0]) }''', ORIGIN),
    'different-source': ('''fn bad(a: &[]i32, b: &[]i32) -> &[]i32 from a { return b }''', ORIGIN),
    'different-index-source': ('''fn bad(a: &[]i32, b: &[]i32) -> &i32 from a {
        return &b[a[0]]
    }''', ORIGIN),
    'different-call-source': ('''fn bad(a: &[]i32, b: &[]i32) -> &[]i32 from a {
        return views.prefer(b, a)
    }''', ORIGIN),
    'different-branch-source': ('''fn bad(a: &[]i32, b: &[]i32, yes: bool) -> &[]i32 from a {
        return if yes { &b[0..1] } else { &a[0..1] }
    }''', ORIGIN),
    'different-match-source': ('''fn bad(a: &[]i32, b: &[]i32, s: views.Side) -> &[]i32 from a {
        return match s { .Back => a, .Front => { nested := &b[0..1]; nested } }
    }''', ORIGIN),
    'different-method-source': ('''struct Pair { data: []i32 }
    impl Pair {
        fn bad(&self, other: &Pair) -> &[]i32 from self { &other.data }
    }''', ORIGIN),
    'missing-source': ('fn bad() -> &i32 { local := 42; &local }', 'requires a shared input'),
    'ambiguous': ('fn bad(a: &i32, b: &i32) -> &i32 { a }', 'ambiguous returned borrow'),
    'unknown-source': ('''fn bad(a: &i32) -> &i32 from missing { a }''', 'unknown returned borrow source'),
    'moved-source': ('''fn bad(a: &[]i32, move b: []i32) -> &[]i32 from b { &b }''', 'requires a shared input'),
    'exclusive-source': ('''fn bad(a: &i32, b: &mut i32) -> &i32 from b { &b }''', 'requires a shared input'),
    'mutable-result': ('fn bad(a: &mut i32) -> &mut i32 { &mut a }', 'cannot return a mutable borrow'),
    'foreign-result': ('fn bad(f: extern fn(i32) -> &i32) {}', 'C calls cannot return a Dunder borrow'),
    'callback-source-index': ('fn bad(f: fn(&i32, &i32) -> &i32 from 3) {}', 'source parameter number is out of range'),
}

INVALID_CLIENTS = {
    'bound-temporary': ('r := views.window([42], 0, 1); print(r[0])', 'local borrow requires a named owner'),
    'bound-temporary-field': ('''r := views.first(make_fields().data); print(r)''', 'local borrow requires a named owner'),
    'mutation': ('r := views.first(v); v[0] = 7; print(r)', 'borrow'),
    'reallocation': ('r := views.window(v, 0, 1); v.reserve(128); print(r[0])', 'borrow'),
    'transitive-alias': ('r := views.window(v, 0, 2); s := views.first(r); v.clear(); print(s)', 'borrow'),
    'move': ('r := views.window(v, 0, 1); moved := v; print(r[0])', 'borrow'),
    'replaced-field': ('r := views.window(f.data, 0, 1); f.data = [9]; print(r[0])', 'borrow'),
    'replaced-receiver': ('r := f.middle(); f = make_fields(); print(r[0])', 'borrow'),
    'moved-parent': ('r := views.first(f.data); moved := f; print(r)', 'borrow'),
    'other-argument-moves': ('print(combine(views.window(v, 0, 1), size(move v)))', 'borrow'),
    'mut-argument-first': ('write(&mut v, views.window(v, 0, 1))', 'borrow'),
    'mut-argument-last': ('print(combine(views.window(v, 0, 1), reset(&mut v)))', 'borrow'),
    'index-mutates-source': ('print(views.window(v, 0, 1)[reset(&mut v)])', 'borrow'),
    'loop-mutates-source': ('for n in views.window(v, 0, 1) { v[0] = n }', 'while iterating over it'),
    'match-replaces-owner': ('''mut e := views.Envelope { message: .Text("dunder") }
    match views.envelope(e).message {
        .Text(s) => { e = views.Envelope { message: .Empty }; print(s.len) },
        .Empty => {},
    }''', 'while iterating'),
    'captured-in-defer': ('r := views.first(v); defer { print(r) }; v[0] = 7', 'borrow'),
    'mismatched-signature': ('''choose: fn(&[]i32, &[]i32) -> &[]i32 from 2 = views.prefer''', 'type mismatch'),
    'raw-through-return': ('p := expose(views.window(v, 0, 1))', 'scoped borrow reaches a raw address'),
    'raw-through-temporary': ('print(observe(make_fields().data).len)', 'scoped borrow reaches a raw address'),
    'raw-through-temporary-receiver': ('print(Box { v: [42] }.observe().len)', 'scoped borrow reaches a raw address'),
    'task-result': ('task scope jobs(1) { key := spawn jobs identity(v) }', 'returned borrow'),
}

CLIENT_HELPERS = '''
fn make_fields() -> views.Fields { views.Fields { data: [1, 2, 3], spare: [4], number: 5 } }
fn combine(a: &[]i32, b: i32) -> i32 { a[0] + b }
fn size(move v: []i32) -> i32 { v.len }
fn reset(v: &mut []i32) -> i32 { v = [7]; 0 }
fn write(v: &mut []i32, a: &[]i32) { v = [a[0]] }
fn expose(v: &[]i32) -> ptr { unsafe { addr(&v[0]) } }
fn observe(v: &[]i32) -> &[]i32 { unsafe { p := addr(&v[0]) }; v }
struct Box { v: []i32 }
impl Box { fn observe(&self) -> &[]i32 { unsafe { p := addr(&self.v[0]) }; &self.v } }
fn identity(v: &[]i32) -> &[]i32 { v }
'''


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return 'arm64-macos'
    if host in (('Linux', 'arm64'), ('Linux', 'aarch64')):
        return 'arm64-linux'
    if host == ('Linux', 'x86_64'):
        return 'x86_64-linux'
    raise RuntimeError(f'unsupported execution host {host}')


def run(command, work, *, status=0, output=None):
    command = [str(item) for item in command]
    result = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=90)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, result.stderr)
    return result


def diagnostic(result, source, expected):
    record = json.loads(result.stdout)
    assert record['kind'] == 'error' and expected in record['message'], (source.name, record)
    assert record['path'] == str(source), record
    assert 0 <= record['start'] < record['end'] <= source.stat().st_size, record
    return record


def reject(compiler, source, expected, work, flags):
    result = run([compiler, 'check', source, '--agent', *flags], work, status=1)
    record = diagnostic(result, source, expected)
    image = work / 'previous-image'
    image.write_bytes(b'previous valid image')
    failed = run([compiler, 'build', source, *flags, '-o', image], work, status=1)
    assert record['message'] in failed.stderr, (source.name, record, failed)
    assert image.read_bytes() == b'previous valid image', source.name
    return record


def execute(name, runner, image, work):
    if name in ('values', 'layouts'):
        output = '43\nreturned borrows ok\n' if name == 'values' else 'returned layouts ok\n'
        result = run([*runner, image], work, output=output)
        assert not result.stderr, result
        return 1
    if name == 'faults':
        for arg, (status, expected) in FAULTS.items():
            run([*runner, image, arg], work, status=status, output=expected)
        return len(FAULTS)
    profiles = []
    for arg in ('0', '1'):
        result = run([*runner, image, arg], work, output='60672\n')
        record = json.loads(result.stderr)
        assert record['kind'] == 'runtime_profile' and record['version'] == 1, record
        profiles.append(record['counters'])
    # Both paths allocate the two input owners once. Returning and forwarding
    # views must not allocate, retain, clone or slice an owning string.
    assert profiles[0] == profiles[1], profiles
    assert profiles[0]['allocation_count'] > 0, profiles
    for key in ('array_clone_calls', 'record_clone_calls', 'string_slice_calls'):
        assert profiles[0][key] == 0, (key, profiles)
    return 2


def incremental_source(compiler, work, target, jobs, runners):
    module = work / 'chooser.ddr'
    source = work / 'edit-source.ddr'
    image = work / 'edit-source'
    flags = ['--target', target, '--jobs', jobs, '--incremental']
    declaration = 'pub fn pick(a: &[]i32, b: &[]i32) -> &[]i32 from {source} {{ {source} }}\n'
    client = '''import chooser
fn main() {{
    mut a: []i32 = [20]
    mut b: []i32 = [22]
    view := chooser.pick(a, b)
    {other}[0] = 0
    print(view[0])
}}
'''
    module.write_text(declaration.format(source='a'))
    source.write_text(client.format(other='b'))
    run([compiler, 'build', source, *flags, '-o', image], work)
    good = image.read_bytes()
    run([compiler, 'build', source, *flags, '-o', image], work)
    assert image.read_bytes() == good, (target, 'warm image')
    if target in runners:
        run([*runners[target], image], work, output='20\n')
    # Only the imported contract/body changes. A warm caller must now reject
    # its write to b and preserve the last working executable.
    module.write_text(declaration.format(source='b'))
    changed = run([compiler, 'build', source, *flags, '-o', image], work, status=1)
    checked = run([compiler, 'check', source, '--agent', '--target', target, '--jobs', jobs],
                  work, status=1)
    record = diagnostic(checked, source, 'borrow')
    assert record['message'] in changed.stderr and image.read_bytes() == good, (target, changed)
    source.write_text(client.format(other='a'))
    run([compiler, 'build', source, *flags, '-o', image], work)
    fresh = image.read_bytes()
    run([compiler, 'build', source, '--target', target, '--jobs', 1, '-o', image], work)
    assert image.read_bytes() == fresh, (target, 'changed cache image')
    if target in runners:
        run([*runners[target], image], work, output='22\n')


def large_stack_source(work):
    # The last source argument and the hidden slice-result pointer both lie
    # beyond ARM64's largest scaled immediate stack-load offset (32760).
    parameters = ', '.join(f'p{i}: i32' for i in range(1, 2056))
    arguments = ', '.join(str(i) for i in range(1, 2056))
    source = work / 'many-arguments.ddr'
    source.write_text(f'''fn window(data: &[]i32, {parameters}) -> &[]i32 {{
    assert(p2055 == 2055)
    &data[1..2]
}}
fn main() {{
    data: []i32 = [7, 42]
    r := window(data, {arguments})
    assert(r.len == 1)
    print(r[0])
}}
''')
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a native target through a local wrapper')
    parser.add_argument('--serial', action='store_true',
                        help='use jobs 1 only for the serial bootstrap compiler')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    runners = {host_target(): []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    workers = (1,) if args.serial else (1, 4)
    builds = executions = rejections = 0
    with tempfile.TemporaryDirectory(prefix='dunder-returned-borrows-') as directory:
        work = Path(directory).resolve()
        shutil.copy(FIXTURES / 'views.ddr', work / 'views.ddr')
        negatives = {}
        for name, (body, message) in INVALID_DECLARATIONS.items():
            source = work / f'invalid-{name}.ddr'
            source.write_text('capability unsafe\nimport views\n' + body + '\nfn main() {}\n')
            negatives[source] = message
        for name, (body, message) in INVALID_CLIENTS.items():
            source = work / f'invalid-{name}.ddr'
            source.write_text('capability unsafe\nimport views\n' + CLIENT_HELPERS
                              + 'fn main() { mut v: []i32 = [20, 22]; mut f := make_fields()\n'
                              + body + '\n}\n')
            negatives[source] = message
        records = {}
        wide = large_stack_source(work)
        for target in TARGETS:
            for mode in MODES:
                flags = ['--target', target, *mode]
                for name in ('values', 'layouts', 'profile', 'faults'):
                    source = FIXTURES / f'{name}.ddr'
                    checked = run([compiler, 'check', source, '--agent', *flags], work)
                    assert json.loads(checked.stdout.splitlines()[-1])['kind'] == 'ok', checked
                    profile = ['--runtime-profile'] if name == 'profile' else []
                    image = work / name
                    previous = None
                    for jobs in workers:
                        run([compiler, 'build', source, *flags, '--jobs', jobs, *profile, '-o', image], work)
                        data = image.read_bytes()
                        assert data[:4] == (b'\xcf\xfa\xed\xfe' if target == TARGETS[0] else b'\x7fELF')
                        if previous is not None:
                            assert previous == data, (name, target, 'worker images differ')
                        if jobs == 1 and target in runners:
                            executions += execute(name, runners[target], image, work)
                        previous = data
                        builds += 1
                for source, message in negatives.items():
                    record = reject(compiler, source, message, work, [*flags, '--jobs', workers[-1]])
                    assert records.setdefault(source, record) == record, (source, records[source], record)
                    rejections += 1
            incremental_source(compiler, work, target, workers[-1], runners)
            image = work / 'many-arguments'
            run([compiler, 'build', wide, '--target', target, '--jobs', workers[-1], '-o', image], work)
            builds += 1
            if target in runners:
                run([*runners[target], image], work, output='42\n')
                executions += 3
    print(f'Returned borrows passed {builds} native images, {executions} executions '
          f'({", ".join(runners)}), {rejections} check/build rejections and '
          f'{len(TARGETS)} incremental source changes')


if __name__ == '__main__':
    main()
