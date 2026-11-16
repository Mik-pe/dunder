#!/usr/bin/env python3
"""Verify lexical read loans and allocation-free views on native runtimes."""
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
MODES = ((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'),
         ('--mir-ssa=on',))
TYPES = '''
struct Pair { left: []i32, right: []i32, number: i32, text: string }
struct Outer { pair: Pair, tail: []i32 }
fn pair() -> Pair { Pair { left: [20, 22], right: [7], number: 42, text: "dunder" } }
fn sum(values: &[]i32) -> i32 {
    mut result := 0
    for value in values { result += value }
    result
}
fn read(value: &i32) -> i32 { value }
fn text_length(value: &string) -> i32 { value.len }
'''

LOCALS = TYPES + '''
fn reborrow(value: &mut Pair) -> i32 {
    {
        left := &value.left
        alias := &left
        value.number = 9
        assert(sum(left) == 42 && sum(alias) == 42 && value.number == 9)
    }
    value.left = [40, 2]
    sum(value.left)
}
fn early(flag: bool) -> i32 {
    mut values: []i32 = [20, 22]
    {
        first := &values[0]
        if flag { return read(first) + values[1] }
    }
    values[0] = 21
    values[0] + values[1] - 1
}
fn owned_copy(values: &[]i32) -> []i32 {
    view := &values[0..values.len]
    view.clone()
}
fn main() {
    mut value := pair()
    {
        left := &value.left
        alias := &left
        number: &i32 = &value.number
        label := &value.text
        // A loan of one field permits moves and writes to its siblings.
        right := value.right
        assert(right[0] == 7)
        value.right = [8]
        assert(sum(left) == 42 && sum(alias) == 42 && read(number) == 42)
        assert(text_length(label) == 6 && label[0] == 100)
    }
    value.left = [41, 1]
    assert(reborrow(&mut value) == 42)
    mut outer := Outer { pair: pair(), tail: [9] }
    {
        nested := &outer.pair.left
        right := outer.pair.right
        outer.pair.right = [right[0] + 1]
        outer.tail[0] = 10
        assert(sum(nested) == 42 && outer.pair.right[0] == 8)
    }
    mut values: []i32 = [20, 22]
    independent := owned_copy(values)
    for turn in 0..20 {
        {
            first := &values[0]
            assert(first == 20)
        }
        if turn % 2 == 0 { continue }
        values[0] = 20
    }
    {
        view := &values[0..2]
        defer { assert(sum(view) == 42) }
    }
    values[0] = 99
    assert(independent[0] == 20 && independent[1] == 22)
    {
        first := &values[0]
        if first == 99 { nested := &first; assert(nested == 99) }
    }
    moved := values
    assert(moved[0] == 99 && early(true) == 42 && early(false) == 42)
    print("local loans ok")
}
'''

VIEWS = '''capability unsafe
import std.bytes as bytes
''' + TYPES + '''
@c_layout struct Packed { tag: u8, values: [i32; 3] }
fn sliding(values: &[]i32) -> i32 {
    mut total := 0
    for start in 0..values.len {
        window := &values[start..start + 1]
        total += window[0]
    }
    total
}
fn lower(trace: &mut i32) -> i32 { trace = trace * 10 + 1; 1 }
fn upper(trace: &mut i32) -> i32 { trace = trace * 10 + 2; 3 }
fn main() {
    mut values: []i32 = [7, 20, 22, 9]
    values.reserve(64)
    unsafe {
        original := addr(&values[1])
        {
            middle := &values[1..3]
            nested := &middle[1..2]
            empty := &values[values.len..values.len]
            assert(middle.len == 2 && middle.cap() == 2 && sum(middle) == 42)
            assert(nested.len == 1 && nested[0] == 22 && empty.len == 0)
            assert(addr(&middle[0]) == original && addr(&nested[0]) == addr(&values[2]))
        }
        values[1] = 21
    }
    assert(sliding(values) == 59)
    mut trace := 0
    {
        evaluated := &values[lower(&mut trace)..upper(&mut trace)]
        assert(trace == 12 && sum(evaluated) == 43)
    }
    fixed: [i32; 4] = [7, 20, 22, 9]
    fixed_view := &fixed[1..3]
    assert(sum(fixed_view) == 42)
    packed := Packed { tag: 255, values: [7, 20, 22] }
    packed_view := &packed.values[1..3]
    packed_element := &packed.values[2]
    tag := &packed.tag
    assert(sum(packed_view) == 42 && packed_element == 22 && tag == 255)
    text := "x" + "å🌍" + "y"
    unsafe {
        // String bytes are inline after their length word. No temporary
        // string handle or copied byte buffer may replace this original data.
        original := ptr(usize(load_ptr(addr(&text))) + 9)
        utf8 := &text[1..7]
        assert(utf8.len == 6 && bytes.equals_text(utf8, 0, utf8.len, "å🌍"))
        assert(addr(&utf8[0]) == original)
        empty := &text[text.len..text.len]
        assert(empty.len == 0)
    }
    print("borrowed views ok")
}
'''

TEMPORARIES = TYPES + '''
fn fresh() -> []i32 { [20, 22] }
fn disturbance() -> []i32 { [1, 2] }
fn owned_after_read(values: &[]i32) -> []i32 { [99] }
fn text_after_read(values: &[]i32) -> string { 99.str() }
fn first(values: &[]i32) -> i32 { values[0] }
fn length(values: &[]i32) -> i32 { values.len }
fn offset() -> i32 { 57 }
fn combine(values: &[]i32, other: &[]i32) -> i32 { sum(values) + sum(other) }
fn after_allocation(values: &[]i32) -> i32 {
    other: []i32 = [1, 2]
    answer := sum(values)
    assert(other[0] == 1 && other[1] == 2)
    answer
}
fn text_after_allocation(value: &string) -> i32 {
    other := 1000.str()
    assert(other.len == 4)
    value.len
}
fn main() {
    assert(sum(&fresh()) == 42 && sum(&[20, 22]) == 42)
    assert(read(&42) == 42 && read(&(20 + 22)) == 42)
    assert(text_after_allocation(&42.str()) == 2)
    source: [i32; 1] = [42]
    // Nested call arguments can carry a view's frame home. A newly owned
    // result, array constructor or scalar expression must publish its own
    // provenance rather than reusing that inner argument's borrowed storage.
    assert(first(&owned_after_read(&source[0..1])) == 99)
    assert(length(&[99, first(&source[0..1])]) == 2)
    assert(read(&(first(&source[0..1]) + 57)) == 99)
    assert(read(&(first(&source[0..1]) + offset())) == 99)
    assert(text_after_allocation(&text_after_read(&source[0..1])) == 2)
    for turn in 0..2000 {
        // Later argument allocation must not invalidate an earlier temporary.
        assert(combine(&fresh(), &disturbance()) == 45)
        assert(after_allocation(&pair().left) == 42)
        assert(read(&[20, 22][1]) == 22)
    }
    print("temporary call loans ok")
}
'''

PROFILE_HEADER = '''
fn sum(values: &[]i32) -> i32 {
    mut result := 0
    for value in values { result += value }
    result
}
fn main() {
    values: []i32 = [7, 20, 22, 9]
    text := "xå🌍y"
    mut total := 0
    for turn in 0..10000 {
'''
PROFILE_TAIL = '''
    }
    assert(total == 640000)
    print("view allocation oracle ok")
}
'''
PROFILE_CONTROL = PROFILE_HEADER + '''
        total += values[1] + values[2] + values[2]
        assert(text.len == 8 && text[1] == 195 && text[2] == 165 && text[6] == 141)
''' + PROFILE_TAIL
PROFILE_VIEWS = PROFILE_HEADER + '''
        middle := &values[1..3]
        nested := &middle[1..2]
        empty := &values[values.len..values.len]
        utf8 := &text[1..7]
        total += sum(middle) + nested[0] + empty.len
        assert(utf8.len == 6 && utf8[0] == 195 && utf8[1] == 165 && utf8[5] == 141)
''' + PROFILE_TAIL

INVALID = (
    ('move-owner', 'fn main(){v:=pair();r:=&v.left;moved:=v;print(sum(r))}', 'borrow'),
    ('move-field', 'fn main(){v:=pair();r:=&v.left;moved:=v.left;print(sum(r))}', 'borrow'),
    ('move-nested-parent', 'fn main(){v:=Outer{pair:pair(),tail:[7]};r:=&v.pair.left;moved:=v.pair;print(sum(r))}', 'borrow'),
    ('replace-owner', 'fn main(){mut v:=pair();r:=&v.left;old:=replace(&mut v,pair());print(sum(r))}', 'borrow'),
    ('replace-field', 'fn main(){mut v:=pair();r:=&v.left;old:=replace(&mut v.left,[42]);print(sum(r))}', 'borrow'),
    ('write-field', 'fn main(){mut v:=pair();r:=&v.left;v.left=[42];print(sum(r))}', 'borrow'),
    ('write-element', 'fn main(){mut v:[]i32=[20,22];r:=&v[0];v[0]=21;print(r)}', 'borrow'),
    ('write-other-element', 'fn main(){mut v:[]i32=[20,22];r:=&v[0];v[1]=21;print(r)}', 'borrow'),
    ('grow-owner', 'fn main(){mut v:[]i32=[20,22];r:=&v[0..1];v.reserve(64);print(sum(r))}', 'borrow'),
    ('pop-owner', 'fn main(){mut v:[]i32=[20,22];r:=&v[0];old:=v.pop();print(r)}', 'borrow'),
    ('mut-call', 'fn write(v:&mut Pair){v.left=[42]} fn main(){mut v:=pair();r:=&v.left;write(&mut v);print(sum(r))}', 'borrow'),
    ('mutable-alias', 'fn main(){mut v:[]i32=[20,22];r:=&v;old:=replace(&mut r,[42])}', 'mut'),
    ('alias-chain', 'fn main(){mut v:[]i32=[20,22];r:=&v;s:=&r;v[0]=42;print(sum(s))}', 'borrow'),
    ('lexical-after-last-read', 'fn main(){mut v:[]i32=[42];r:=&v;print(sum(r));v[0]=7}', 'borrow'),
    ('outer-loan-in-branch', 'fn bad(flag:bool){mut v:[]i32=[42];r:=&v;if flag{v[0]=7};print(sum(r))} fn main(){}', 'borrow'),
    ('outer-loan-in-loop', 'fn main(){mut v:[]i32=[42];r:=&v;for i in 0..2{v[0]=i};print(sum(r))}', 'borrow'),
    ('deferred-loan', 'fn main(){mut v:[]i32=[42];r:=&v;defer{print(sum(r))};v[0]=7}', 'borrow'),
    ('view-argument-mutation', 'fn reset(v:&mut []i32)->i32{v=[99];0} fn take(v:&[]i32,n:i32)->i32{v[0]+n} fn main(){mut v:[]i32=[42];print(take(&v[0..1],reset(&mut v)))}', 'borrow'),
    ('view-argument-move', 'fn size(move v:[]i32)->i32{v.len} fn take(v:&[]i32,n:i32)->i32{v[0]+n} fn main(){v:[]i32=[42];print(take(&v[0..1],size(move v)))}', 'borrow'),
    ('view-mutable-argument-first', 'fn take(a:&mut []i32,b:&[]i32){} fn main(){mut v:[]i32=[42];take(&mut v,&v[0..1])}', 'borrow'),
    ('view-mutable-argument-last', 'fn take(a:&[]i32,b:&mut []i32){} fn main(){mut v:[]i32=[42];take(&v[0..1],&mut v)}', 'borrow'),
    ('bound-mutates-source', 'fn reset(v:&mut []i32)->i32{v=[99];0} fn main(){mut v:[]i32=[42];r:=&v[reset(&mut v)..1];print(sum(r))}', 'borrow'),
    ('dynamic-bound-provenance', 'fn lower(trace:&mut i32)->i32{trace+=1;0} fn main(){mut v:[]i32=[42];mut trace:=0;r:=&v[lower(&mut trace)..1];v[0]=7;print(sum(r))}', 'borrow'),
    ('local-temporary-array', 'fn main(){r:=&[20,22];print(sum(r))}', 'borrow'),
    ('local-temporary-call', 'fn fresh()->[]i32{[20,22]} fn main(){r:=&fresh();print(sum(r))}', 'borrow'),
    ('local-mutable-reference', 'fn main(){mut v:[]i32=[42];r:=&mut v;print(sum(r))}', 'borrow'),
    ('return-reference', 'fn bad(v:&i32)->&i32{&v} fn main(){}', 'borrow'),
    ('return-borrowed-owner', 'fn bad(v:[]i32)->[]i32{r:=&v;r} fn main(){}', 'borrow'),
    ('return-borrowed-view', 'fn bad(v:[]i32)->[]i32{r:=&v[0..v.len];r} fn main(){}', 'borrow'),
    ('reference-field', 'struct Bad{value:&i32} fn main(){}', 'borrow'),
    ('reference-payload', 'enum Bad{Value(&i32),Empty} fn main(){}', 'payload'),
    ('reference-array', 'fn main(){v:=42;items:[]&i32=[&v]}', 'aggregate'),
)

# A scoped reference may be observed directly inside explicit unsafe code, as
# the identity oracle does, but cannot be hidden behind a raw-exposing callee.
RAW_INVALID = (
    ('raw-exposing-callee', 'fn expose(v:&[]i32)->ptr{unsafe{addr(&v[0])}} fn main(){v:[]i32=[42];r:=&v[0..1];p:=expose(r)}', 'scoped borrow reaches a raw address'),
    ('raw-exposing-indirect', 'fn expose(v:&i32)->ptr{unsafe{addr(&v)}} fn relay(f:fn(&i32)->ptr,v:&i32)->ptr{f(v)} fn main(){v:=42;r:=&v;p:=relay(expose,r)}', 'scoped borrow reaches a raw address'),
)

FAULTS = (
    ('array-negative-start', 'values:[]i32=[42];r:=&values[-1..1];print(sum(r))', 103, ''),
    ('array-negative-end', 'values:[]i32=[42];r:=&values[0..-1];print(sum(r))', 103, ''),
    ('array-reversed', 'values:[]i32=[20,22];r:=&values[2..1];print(sum(r))', 103, ''),
    ('array-past-end', 'values:[]i32=[42];r:=&values[0..2];print(sum(r))', 103, ''),
    ('text-past-end-before-unicode', 'text:="å";r:=&text[1..3];print(r.len)', 103, ''),
    ('text-start-boundary', 'text:="å🌍";r:=&text[1..6];print(r.len)', 106, ''),
    ('text-end-boundary', 'text:="å🌍";r:=&text[0..3];print(r.len)', 106, ''),
    ('text-empty-boundary', 'text:="å";r:=&text[1..1];print(r.len)', 106, ''),
    ('endpoints-before-call', 'values:[]i32=[42];print(sum(&values[first()..last()]))', 103, '1\n2\n'),
)

TASK_VALID = '''
fn values(n: i32) -> []i32 { [n] }
fn number(n: i32) -> i32 { n }
fn first(values: &[]i32) -> i32 { values[0] }
fn scalar(value: &i32) -> i32 { value }
fn main() {
    task scope jobs(2) {
        array_key := spawn jobs values(42)
        assert(first(&await! jobs(array_key)) == 42)
        number_key := spawn jobs number(42)
        assert(scalar(&await jobs(number_key)) == 42)
        assert(scalar(&await! jobs(number_key)) == 42)
    }
    print("task temporary loans ok")
}
'''

TASK_INVALID = (
    ('await-owned-escape', '''capability unsafe
fn worker(n: i32) -> []i32 { [n] }
fn expose(values: &[]i32) -> ptr { unsafe { addr(&values[0]) } }
fn main() { task scope jobs(1) {
    key := spawn jobs worker(42)
    leaked := expose(&await! jobs(key))
} }
'''),
    ('await-scalar-escape', '''capability unsafe
fn worker(n: i32) -> i32 { n }
fn expose(value: &i32) -> ptr { unsafe { addr(&value) } }
fn main() { task scope jobs(1) {
    key := spawn jobs worker(42)
    leaked := expose(&await jobs(key))
} }
'''),
    ('spawn-key-escape', '''capability unsafe
fn worker(n: i32) -> i32 { n }
fn expose(value: &TaskID<i32>) -> ptr { unsafe { addr(&value) } }
fn main() { task scope jobs(1) {
    leaked := expose(&spawn jobs worker(42))
} }
'''),
)


def host_target():
    system, machine = platform.system(), platform.machine()
    if (system, machine) == ('Darwin', 'arm64'):
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if (system, machine) == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host {system}/{machine}')


def run(command, cwd, *, status=0, output=None):
    result = subprocess.run(list(map(str, command)), cwd=cwd, capture_output=True,
                            text=True, timeout=90)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, result.stderr)
    return result


def counters(stderr):
    record = json.loads(stderr)
    assert record['kind'] == 'runtime_profile' and record['version'] == 1, record
    return record['counters']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a native target through a local emulator or wrapper')
    parser.add_argument('--serial', action='store_true',
                        help='use jobs 1 only for a compiler without the parallel entry')
    parser.add_argument('--only-tasks', action='store_true',
                        help='run only the ARM64/macOS task-result borrow regressions')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    if args.only_tasks and host != TARGETS[0]:
        parser.error('--only-tasks requires the ARM64/macOS task runtime')
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)

    def jobs_for(target):
        return (1, 4) if target == host == TARGETS[0] and not args.serial else (1,)

    builds = executed = errors = 0
    programs = () if args.only_tasks else (('locals', LOCALS, 'local loans ok\n'),
                ('views', VIEWS, 'borrowed views ok\n'),
                ('temporaries', TEMPORARIES, 'temporary call loans ok\n'),
                ('profile-control', PROFILE_CONTROL, 'view allocation oracle ok\n'),
                ('profile-views', PROFILE_VIEWS, 'view allocation oracle ok\n'))
    with tempfile.TemporaryDirectory(prefix='dunder-local-borrows-') as temporary:
        work = Path(temporary).resolve()

        def rejected(source, message, target):
            nonlocal errors
            diagnostics = []
            for mode in MODES:
                for jobs in jobs_for(target):
                    flags = ['--target', target, '--jobs', jobs, *mode]
                    checked = run([compiler, 'check', source, '--agent', *flags], work, status=1)
                    record = json.loads(checked.stdout)
                    assert record['kind'] == 'error' and message in record['message'], (source.stem, record)
                    assert record['path'] == str(source), record
                    assert 0 <= record['start'] < record['end'] <= len(source.read_bytes()), record
                    diagnostics.append(record)
                    image = work / 'previous-image'
                    image.write_bytes(b'previous valid image')
                    built = run([compiler, 'build', source, *flags, '-o', image], work, status=1)
                    assert record['message'] in built.stderr, (record, built.stderr)
                    assert image.read_bytes() == b'previous valid image'
            assert all(record == diagnostics[0] for record in diagnostics), diagnostics
            errors += 1

        profiles = {}
        for name, source_text, output in programs:
            source = work / (name + '.ddr')
            source.write_text(source_text)
            for target in TARGETS:
                for mode in MODES:
                    for jobs in jobs_for(target):
                        image = work / f'{name}-{builds}'
                        run([compiler, 'build', source, '--target', target, '--jobs', jobs,
                             '--runtime-profile', *mode, '-o', image], ROOT)
                        if target in runners:
                            result = run([*runners[target], image], work, output=output)
                            if name.startswith('profile-'):
                                profile = counters(result.stderr)
                                for counter in ('retain_calls', 'array_clone_calls',
                                                'record_clone_calls', 'string_slice_calls'):
                                    assert profile[counter] == 0, (name, target, mode, profile)
                                key = (target, mode, jobs)
                                if name == 'profile-control':
                                    profiles[key] = profile
                                else:
                                    control = profiles[key]
                                    # The owner is constructed in both programs.
                                    # Creating 40,000 views adds no heap events.
                                    for counter in ('allocation_count', 'requested_bytes',
                                                    'size_class_payload_bytes', 'fresh_allocations',
                                                    'reused_allocations', 'record_construct_calls'):
                                        assert profile[counter] == control[counter], (counter, control, profile)
                            elif name == 'temporaries':
                                profile = counters(result.stderr)
                                assert profile['fresh_allocations'] < 128, profile
                                assert profile['reused_allocations'] > 2000, profile
                            executed += 1
                        builds += 1
        for name, body, message in (() if args.only_tasks else INVALID + RAW_INVALID):
            source = work / ('invalid-' + name + '.ddr')
            capability = 'capability unsafe\n' if name.startswith('raw-exposing-') else ''
            source.write_text(capability + TYPES + body + '\n')
            for target in TARGETS:
                rejected(source, message, target)
        for name, body, status, output in (() if args.only_tasks else FAULTS):
            source = work / ('fault-' + name + '.ddr')
            source.write_text(TYPES + 'fn first()->i32{print(1);-1} '
                              'fn last()->i32{print(2);1} fn main(){' + body + '}\n')
            for target in TARGETS:
                for mode in MODES:
                    for jobs in jobs_for(target):
                        image = work / f'fault-{builds}'
                        run([compiler, 'build', source, '--target', target,
                             '--jobs', jobs, *mode, '-o', image], ROOT)
                        if target in runners:
                            run([*runners[target], image], work, status=status, output=output)
                            executed += 1
                        builds += 1
        if host == TARGETS[0]:
            # The task runtime is ARM64/macOS-only and rejects runtime
            # profiling. Its fresh-expression producers still need the same
            # scoped-loan escape analysis as calls and constructors.
            source = work / 'task-temporary.ddr'
            source.write_text(TASK_VALID)
            for mode in MODES:
                for jobs in jobs_for(host):
                    image = work / f'task-temporary-{builds}'
                    run([compiler, 'build', source, '--target', host, '--jobs', jobs,
                         *mode, '-o', image], ROOT)
                    run([image], work, output='task temporary loans ok\n')
                    builds += 1
                    executed += 1
            for name, source_text in TASK_INVALID:
                source = work / ('invalid-' + name + '.ddr')
                source.write_text(source_text)
                rejected(source, 'scoped borrow reaches a raw address', host)
    print(f'local borrows: {builds} native images, {executed} target executions, '
          f'{errors} rejected contracts; lexical loans, view identity/allocation, '
          'temporary lifetimes, bounds/UTF-8 and diagnostic/publication parity passed')


if __name__ == '__main__':
    main()
