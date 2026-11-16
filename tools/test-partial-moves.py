#!/usr/bin/env python3
"""Verify field ownership, initialized replacement and cleanup on native targets."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
MODES = ((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))
TYPES = '''
struct Pair { first: []i32, second: []i32, label: string }
struct Nested { pair: Pair, extra: []i32 }
@resource struct Handle { identity: i32 }
enum Payload { Numbers([]i32), Empty }
struct Mixed { payload: Payload, handle: Handle, tail: string }
fn pair(a: i32, b: i32) -> Pair {
    Pair { first: [a], second: [b], label: 42.str() }
}
fn total(value: Pair) -> i32 { value.first[0] + value.second[0] }
'''

PARTIAL = '''capability unsafe
''' + TYPES + '''
fn branch(flag: bool) -> i32 {
    mut value := pair(20, 22)
    first := value.first
    if flag {
        second := value.second; assert(second[0] == 22)
        value.second = [22]
    } else {
        second := value.second; assert(second[0] == 22)
        value.second = [22]
    }
    value.first = first
    total(value)
}
fn terminating_branch(flag: bool) -> i32 {
    value := pair(20, 22)
    if flag { first := value.first; return first[0] + value.second[0] }
    total(value)
}
fn loop_fields() -> i32 {
    mut value := pair(20, 22)
    for turn in 0..20 {
        first := value.first
        assert(first[0] == 20)
        value.first = [20]
        if turn % 2 == 0 { continue }
        second := value.second
        assert(second[0] == 22)
        value.second = [22]
    }
    // The first field is already absent at loop entry. The second field is
    // first mentioned inside the loop and still needs a backedge proof.
    first := value.first
    for turn in 0..3 {
        second := value.second
        assert(second[0] == 22)
        value.second = [22]
    }
    value.first = first
    total(value)
}
fn break_sibling(flag: bool) -> i32 {
    value := pair(20, 22)
    for turn in 0..2 {
        if flag { first := value.first; assert(first[0] == 20); break }
    }
    value.second[0]
}
fn main() {
    mut buffer: []i32 = []
    buffer.reserve(64); buffer.push(20)
    unsafe {
        address := addr(&buffer[0])
        mut value := Nested { pair: Pair { first: buffer, second: [22], label: 42.str() }, extra: [7] }
        first := value.pair.first
        assert(first.cap() == 64 && addr(&first[0]) == address)
        second := value.pair.second
        assert(first[0] + second[0] == 42 && value.extra[0] == 7)
        label := value.pair.label
        assert(label == "42")
        value.pair = pair(20, 22)
        complete := value
        assert(total(complete.pair) == 42 && complete.extra[0] == 7)
    }
    mixed := Mixed { payload: .Numbers([42]), handle: Handle { identity: 9 }, tail: 7.str() }
    payload := mixed.payload
    handle := mixed.handle
    tail := mixed.tail
    assert(handle.identity == 9 && tail == "7")
    match move payload {
        .Numbers(values) => { assert(values[0] == 42) },
        .Empty => { assert(false) },
    }
    // Reading or mutating an independent field remains legal after a move.
    mut restored := pair(20, 22)
    old := restored.first
    restored.second[0] = 21
    restored.first = [old[0] + 1]
    assert(total(restored) == 42)
    assert(branch(true) == 42 && branch(false) == 42)
    assert(terminating_branch(true) == 42 && terminating_branch(false) == 42)
    assert(loop_fields() == 42 && break_sibling(true) == 22 && break_sibling(false) == 22)
    mut trace: []i32 = []
    {
        deferred := pair(20, 22)
        defer { trace.push(total(deferred)) }
    }
    assert(trace.len == 1 && trace[0] == 42)
    print("partial moves ok")
}
'''

REPLACE = '''capability unsafe
''' + TYPES + '''
fn drain(values: &mut []i32) -> []i32 { replace(&mut values, []) }
fn exchange(value: &mut Pair) -> Pair { replace(&mut value, pair(1, 2)) }
fn index(trace: &mut i32) -> i32 { trace = trace * 10 + 1; 0 }
fn replacement(trace: &mut i32) -> string { trace = trace * 10 + 2; 99.str() }
fn main() {
    mut number: i32 = -2147483648
    assert(replace(&mut number, 2147483647) == -2147483648 && number == 2147483647)
    mut small: u8 = 255
    assert(replace(&mut small, 0) == 255 && small == 0)
    mut fraction: f64 = 1.25
    assert(replace(&mut fraction, -3.5) == 1.25 && fraction == -3.5)
    mut text := 42.str()
    old_text := replace(&mut text, 7.str())
    assert(old_text == "42" && text == "7")
    mut values: []i32 = []
    values.reserve(64); values.push(42)
    unsafe {
        address := addr(&values[0])
        drained := drain(&mut values)
        assert(values.len == 0 && drained.cap() == 64 && addr(&drained[0]) == address)
        assert(drained[0] == 42)
        new_values: []i32 = [20, 22]
        new_address := addr(&new_values[0])
        empty := replace(&mut values, move new_values)
        assert(empty.len == 0 && addr(&values[0]) == new_address)
    }
    mut owner := pair(20, 22)
    old_owner := exchange(&mut owner)
    assert(total(old_owner) == 42 && total(owner) == 3)
    old_field := replace(&mut owner.first, [40])
    assert(old_field[0] == 1 && total(owner) == 42)
    mut payload := Payload.Numbers([42])
    old_payload := replace(&mut payload, .Empty)
    match move old_payload {
        .Numbers(numbers) => { assert(numbers[0] == 42) },
        .Empty => { assert(false) },
    }
    match move payload { .Empty => {}, .Numbers(numbers) => { assert(false) }, }
    mut handle := Handle { identity: 42 }
    old_handle := replace(&mut handle, Handle { identity: 7 })
    assert(old_handle.identity == 42 && handle.identity == 7)
    mut words: []string = [20.str(), 22.str()]
    mut trace := 0
    first := replace(&mut words[index(&mut trace)], replacement(&mut trace))
    assert(trace == 12 && first == "20" && words[0] == "99" && words[1] == "22")
    mut bytes: []u8 = [255, 7]
    assert(replace(&mut bytes[0], 0) == 255 && bytes[0] == 0 && bytes[1] == 7)
    // Ignoring an old owner still creates one temporary cleanup responsibility.
    _ := replace(&mut words[1], 42.str())
    assert(words[1] == "42")
    print("replace ok")
}
'''

# Inline C and fixed-POD references have a different ABI from descriptor slots.
# Keep their oracle separate so its admission is an explicit language contract.
INLINE_REPLACE = '''
@c_layout struct Inline { first: u8, values: [i32; 2] }
fn fixed(values: &mut [i32; 2]) -> [i32; 2] { replace(&mut values, [1, 2]) }
fn inline(value: &mut Inline) -> Inline { replace(&mut value, Inline { first: 7, values: [3, 4] }) }
fn main() {
    mut values: [i32; 2] = [20, 22]
    old := fixed(&mut values)
    assert(old[0] + old[1] == 42 && values[0] == 1 && values[1] == 2)
    mut value := Inline { first: 255, values: [20, 22] }
    prior := inline(&mut value)
    assert(prior.first == 255 && prior.values[0] + prior.values[1] == 42)
    assert(value.first == 7 && value.values[0] == 3 && value.values[1] == 4)
    old_values := replace(&mut value.values, [8, 9])
    assert(old_values[0] == 3 && old_values[1] == 4 && value.values[1] == 9)
    assert(replace(&mut value.first, 255) == 7 && value.first == 255)
    assert(replace(&mut value.values[0], 42) == 8 && value.values[0] == 42)
    mut source := Inline { first: 5, values: [20, 22] }
    moved := Inline { first: 6, values: source.values }
    assert(source.first == 5 && moved.values[0] + moved.values[1] == 42)
    source.values = [8, 9]
    assert(source.values[0] == 8 && source.values[1] == 9)
    source = moved
    assert(source.first == 6 && source.values[0] + source.values[1] == 42)
    print("inline replace ok")
}
'''

CLEANUP = '''
struct Buffers { first: []u8, second: []u8, tail: []u8 }
fn buffer() -> []u8 {
    mut values: []u8 = []; values.reserve(65536); values.push(42); values
}
fn exit_partial(mode: i32) -> i32 {
    mut result := 0
    for turn in 0..2 {
        owner := Buffers { first: buffer(), second: buffer(), tail: buffer() }
        first := owner.first
        assert(first[0] == 42)
        if mode == 0 { return i32(first[0]) }
        second := owner.second
        result = i32(first[0]) + i32(second[0])
        if mode == 1 { continue }
        break
    }
    result
}
fn exit_replaced(mode: i32) -> i32 {
    mut result := 0
    for turn in 0..2 {
        mut owner := Buffers { first: buffer(), second: buffer(), tail: buffer() }
        previous := replace(&mut owner.first, buffer())
        first := owner.first
        assert(first[0] == 42 && previous[0] == 42)
        if mode == 0 { return i32(previous[0]) }
        second := owner.second
        result = i32(previous[0]) + i32(second[0])
        if mode == 1 { continue }
        break
    }
    result
}
fn exit_deferred() -> i32 {
    owner := Buffers { first: buffer(), second: buffer(), tail: buffer() }
    first := owner.first
    defer { assert(owner.second[0] == 42 && owner.tail[0] == 42) }
    i32(first[0])
}
fn main() {
    // Each field allocates 64 KiB. A field leaked per call exceeds the runtime
    // pool; the profile additionally requires a bounded set of fresh blocks.
    for turn in 0..10000 {
        mode := turn % 3
        mut expected := 84; if mode == 0 { expected = 42 }
        assert(exit_partial(mode) == expected && exit_replaced(mode) == expected)
        assert(exit_deferred() == 42)
    }
    print("partial cleanup ok")
}
'''

INVALID = (
    ('field-twice', 'fn main(){v:=pair(20,22);a:=v.first;b:=v.first}', 'moved value'),
    ('whole-read', 'fn main(){v:=pair(20,22);a:=v.first;print(total(v))}', 'moved value'),
    ('whole-move', 'fn main(){v:=pair(20,22);a:=v.first;b:=v}', 'moved value'),
    ('whole-clone', 'fn main(){v:=pair(20,22);a:=v.first;b:=v.clone()}', 'moved value'),
    ('nested-parent', 'fn main(){v:=Nested{pair:pair(20,22),extra:[7]};a:=v.pair.first;b:=v.pair}', 'moved value'),
    ('moved-parent', 'fn main(){v:=Nested{pair:pair(20,22),extra:[7]};a:=v.pair;b:=v.pair.first}', 'moved value'),
    ('branch-field', 'fn bad(flag:bool){v:=pair(20,22);if flag{a:=v.first};print(v.first[0])} fn main(){}', 'moved value'),
    ('branch-whole', 'fn bad(flag:bool){v:=pair(20,22);if flag{a:=v.first}else{b:=v.second};print(total(v))} fn main(){}', 'moved value'),
    ('loop-new-field', 'fn main(){v:=pair(20,22);for i in 0..2{a:=v.first}}', 'moved value'),
    ('loop-new-nested', 'fn main(){v:=Nested{pair:pair(20,22),extra:[7]};for i in 0..2{a:=v.pair.first}}', 'moved value'),
    ('loop-partial-entry', 'fn main(){v:=pair(20,22);a:=v.first;for i in 0..2{b:=v.second}}', 'moved value'),
    ('continue-field', 'fn main(){v:=pair(20,22);for i in 0..2{a:=v.first;continue}}', 'moved value'),
    ('break-field', 'fn main(){v:=pair(20,22);for i in 0..2{a:=v.first;break};print(v.first[0])}', 'moved value'),
    ('borrowed-field', 'fn bad(v:Pair)->[]i32{v.first} fn main(){}', 'borrowed'),
    ('borrowed-mut-field', 'fn bad(v:&mut Pair)->[]i32{v.first} fn main(){}', 'borrowed'),
    ('defer-root', 'fn main(){v:=pair(20,22);defer{print(v.second[0])};a:=v.first}', 'captured by defer'),
    ('field-readonly', 'fn main(){v:=pair(20,22);a:=v.first;v.first=[42]}', 'mutable'),
    ('replace-readonly', 'fn main(){v:[]i32=[42];old:=replace(&mut v,[7])}', 'mut'),
    ('replace-shared', 'fn bad(v:[]i32){old:=replace(&mut v,[7])} fn main(){}', 'mut'),
    ('replace-type', 'fn main(){mut v:[]i32=[42];old:=replace(&mut v,true)}', 'type mismatch'),
    ('replace-explicit-move', 'fn main(){mut a:[]i32=[42];b:[]i32=[7];old:=replace(&mut a,b)}', 'move'),
    ('replace-borrowed-owner', 'fn bad(a:[]i32,b:&mut []i32){old:=replace(&mut b,move a)} fn main(){}', 'borrowed'),
    ('replace-moved-field', 'fn main(){mut v:=pair(20,22);a:=v.first;old:=replace(&mut v.first,[7])}', 'moved value'),
    ('replace-self-move', 'fn main(){mut v:[]i32=[42];old:=replace(&mut v,move v)}', 'borrow'),
    ('replace-root-move', 'fn main(){mut v:=pair(20,22);old:=replace(&mut v.first,move v.second)}', 'borrow'),
    ('replace-root-mutation', 'fn change(v:&mut Pair)->[]i32{v.first=[1];[2]} fn main(){mut v:=pair(20,22);old:=replace(&mut v.first,change(&mut v))}', 'borrow'),
    ('replace-argument-alias', 'fn read(v:[]i32,n:i32)->i32{v[0]} fn main(){mut v:[]i32=[42];print(read(v,replace(&mut v,[7]).len))}', 'borrowed by another argument'),
    ('replace-iterated', 'fn main(){mut v:[]string=["a"];for x in v{old:=replace(&mut v[0],"b")}}', 'iterating'),
    ('replace-region', 'fn main(){region area(64){mut v:[]i32=area.alloc(1);old:=replace(&mut v,[])}}', 'region-backed'),
    ('replace-region-element', 'fn main(){region area(64){mut v:[]i32=area.alloc(1);old:=replace(&mut v[0],7)}}', 'region-backed'),
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


def workers(target, host):
    return (1, 4) if target == host == TARGETS[0] else (1,)


def profile(stderr, *, no_retains, no_clones=True):
    record = json.loads(stderr)
    assert record['kind'] == 'runtime_profile' and record['version'] == 1, record
    counts = record['counters']
    if no_clones:
        assert counts['array_clone_calls'] == counts['record_clone_calls'] == 0, counts
    if no_retains:
        assert counts['retain_calls'] == 0, counts
    return counts


def diagnostic(compiler, source, message, target, host, work):
    records = []
    for mode in MODES:
        for jobs in workers(target, host):
            flags = ['--target', target, '--jobs', jobs, *mode]
            checked = run([compiler, 'check', source, '--agent', *flags], work, status=1)
            record = json.loads(checked.stdout)
            assert record['kind'] == 'error' and message in record['message'], (source.stem, record)
            assert record['path'] == str(source), record
            assert 0 <= record['start'] < record['end'] <= len(source.read_bytes()), record
            records.append(record)
            image = work / 'previous-image'
            image.write_bytes(b'previous valid image')
            built = run([compiler, 'build', source, *flags, '-o', image], work, status=1)
            assert record['message'] in built.stderr, (record, built.stderr)
            assert image.read_bytes() == b'previous valid image'
    assert all(record == records[0] for record in records), records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a target through a local emulator or wrapper; repeat per target')
    parser.add_argument('--inline-replace', action='store_true',
                        help='include replacement of inline C/fixed-POD storage')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    builds = executed = errors = 0
    programs = [('partial', PARTIAL, 'partial moves ok\n'),
                ('replace', REPLACE, 'replace ok\n'),
                ('cleanup', CLEANUP, 'partial cleanup ok\n')]
    if args.inline_replace:
        programs.append(('inline', INLINE_REPLACE, 'inline replace ok\n'))
    with tempfile.TemporaryDirectory(prefix='dunder-partial-moves-') as temporary:
        work = Path(temporary).resolve()
        for name, text, output in programs:
            source = work / (name + '.ddr')
            source.write_text(text)
            for target in TARGETS:
                for mode in MODES:
                    for jobs in workers(target, host):
                        image = work / f'{name}-{target}-{builds}'
                        run([compiler, 'build', source, '--target', target, '--jobs', jobs,
                             '--runtime-profile', *mode, '-o', image], ROOT)
                        if target in runners:
                            result = run([*runners[target], image], work, output=output)
                            # Unoptimized string comparisons can retain their
                            # left operand for evaluation-order safety. The
                            # pure buffer oracle requires zero in both modes.
                            counts = profile(result.stderr, no_retains=not mode or name in ('cleanup', 'inline'),
                                             no_clones=name != 'inline')
                            if name == 'cleanup':
                                assert counts['fresh_allocations'] < 128, counts
                                assert counts['reused_allocations'] > 10000, counts
                            executed += 1
                        builds += 1
        for name, body, message in INVALID:
            source = work / ('invalid-' + name + '.ddr')
            source.write_text(TYPES + body + '\n')
            for target in TARGETS:
                diagnostic(compiler, source, message, target, host, work)
                errors += 1
        # A checked destination, including a side-effecting index, is evaluated
        # before the new owner. An invalid index never evaluates replacement.
        source = work / 'bounds.ddr'
        source.write_text('fn index()->i32{print(1);-1} '
                          'fn fresh()->string{print(2);42.str()} '
                          'fn main(){mut v:[]string=["old"];'
                          '_:=replace(&mut v[index()],fresh());print(3)}\n')
        for target in TARGETS:
            for mode in MODES:
                for jobs in workers(target, host):
                    image = work / f'bounds-{builds}'
                    run([compiler, 'build', source, '--target', target,
                         '--jobs', jobs, *mode, '-o', image], ROOT)
                    if target in runners:
                        run([*runners[target], image], work, status=103, output='1\n')
                        executed += 1
                    builds += 1
    print(f'partial moves: {builds} native images, {executed} target executions, '
          f'{errors} rejected contracts; field joins/backedges, replacement identity, '
          'bounded cleanup and diagnostic/publication parity passed')


if __name__ == '__main__':
    main()
