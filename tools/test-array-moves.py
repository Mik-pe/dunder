#!/usr/bin/env python3
"""Verify owned-array moves, explicit clones and definite ownership at control-flow joins."""
import argparse
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/fixtures/array_moves'
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')

# Each source is valid under the previous implicit-copy model, but must fail
# once a unique array owner has been consumed or an unsupported place is moved.
INVALID = (
    ('local', 'fn main(){a:[]i32=[42];b:=a;print(a[0])}', 'use of moved value'),
    ('length', 'fn main(){a:[]i32=[42];b:=a;print(a.len)}', 'use of moved value'),
    ('clone-moved', 'fn main(){a:[]i32=[42];b:=a;c:=a.clone()}', 'use of moved value'),
    ('borrow-moved', 'fn read(a:&[]i32){} fn main(){a:[]i32=[42];b:=a;read(&a)}', 'use of moved value'),
    ('argument', 'fn take(move a:[]i32){} fn main(){a:[]i32=[42];take(move a);print(a[0])}', 'use of moved value'),
    ('argument-twice', 'fn take(move a:[]i32,move b:[]i32){} fn main(){a:[]i32=[42];take(move a,move a)}', 'use of moved value'),
    ('fixed', 'fn main(){a:[i32;2]=[20,22];b:=a;print(a[0])}', 'use of moved value'),
    ('fixed-empty', 'fn main(){a:[i32;0]=[0;0];b:=a;print(a.len)}', 'use of moved value'),
    ('struct', 'struct P{a:[]i32,n:i32} fn main(){a:=P{a:[42],n:7};b:=a;print(a.n)}', 'use of moved value'),
    ('enum', 'enum E{A([]i32),B} fn main(){a:=E.A([42]);b:=a;match a{.A(v)=>{},.B=>{}}}', 'use of moved value'),
    ('nested', 'struct P{a:[i32;2]} enum E{A(P),B} fn main(){a:=E.A(P{a:[20,22]});b:=a;c:=a}', 'use of moved value'),
    ('field', 'struct P{a:[]i32} fn main(){a:=P{a:[42]};b:=a.a;print(a.a[0])}', 'use of moved value'),
    ('conditional-reference', 'capability unsafe\nfn read(a:&i32)->i32{a} fn main(){a:=42;unsafe{print(read(if true{ref_at(addr(&a))}else{ref_at(addr(&a))}))}}', 'reference'),
    ('repeat-string-owner', 'fn main(){s:="owned"+42.str();values:=[s;4];print(s)}', 'use of moved value'),
    ('repeat-zero-owner', 'fn main(){s:="owned"+42.str();values:=[s;0];print(s)}', 'use of moved value'),
    ('default-parameter-return', 'fn bad(a:[]i32)->[]i32{a} fn main(){}', 'cannot move out of a borrowed value'),
    ('default-parameter-mutation', 'fn bad(a:[]i32){a[0]=99} fn main(){}', 'mutable'),
    ('default-parameter-store', 'fn bad(a:[]i32){owned:=a} fn main(){}', 'cannot move out of a borrowed value'),
    ('callback-mode', 'fn relay(move a:[]i32)->[]i32{a} fn main(){pass:fn([]i32)->[]i32=relay}', 'type mismatch'),
    ('borrowed', 'fn copy(a:&[]i32)->[]i32{a} fn main(){}', 'cannot move out of a borrowed value'),
    ('borrowed-mut', 'fn copy(a:&mut []i32)->[]i32{a} fn main(){}', 'cannot move out of a borrowed value'),
    ('borrowed-field', 'struct P{a:[]i32} fn copy(a:&P)->[]i32{a.a} fn main(){}', 'cannot move out of a'),
    ('if-one', 'fn f(flag:bool){a:[]i32=[42];if flag{b:=a};print(a[0])} fn main(){f(true)}', 'use of moved value'),
    ('if-both', 'fn f(flag:bool){a:[]i32=[42];if flag{b:=a}else{c:=a};print(a[0])} fn main(){f(false)}', 'use of moved value'),
    ('if-expression', 'fn f(flag:bool){a:[]i32=[42];b:=if flag{a}else{[1]};print(a[0])} fn main(){f(true)}', 'use of moved value'),
    ('short-circuit', 'fn take(move a:[]i32)->bool{true} fn f(flag:bool){a:[]i32=[42];b:=flag&&take(move a);print(a[0])} fn main(){f(false)}', 'use of moved value'),
    ('match-join', 'enum E{A,B} fn f(k:E){a:[]i32=[42];match k{.A=>{b:=a},.B=>{}};print(a[0])} fn main(){f(.B)}', 'use of moved value'),
    ('for-consumes', 'fn main(){a:[]i32=[42];for x in move a{print(x)};print(a[0])}', 'use of moved value'),
    ('for-backedge', 'fn main(){a:[]i32=[42];for i in 0..2{b:=a;print(b[0])}}', 'moved value'),
    ('while-backedge', 'fn main(){a:[]i32=[42];mut i:=0;while i<2{b:=a;print(b[0]);i+=1}}', 'moved value'),
    ('continue-backedge', 'fn main(){a:[]i32=[42];for i in 0..2{b:=a;continue}}', 'moved value'),
    ('defer-use', 'fn main(){a:[]i32=[42];defer{print(a[0])};b:=a}', 'captured by defer'),
    ('mutable-moved', 'fn main(){mut a:[]i32=[42];b:=a;a.push(1)}', 'use of moved value'),
    ('defer-consume', 'fn main(){a:[]i32=[42];defer{b:=a}}', 'captured by defer'),
    ('borrowed-loop-move', 'fn take(move a:[]i32){} fn main(){a:[]i32=[42];for x in &a{take(move a)}}', 'while iterating'),
    ('borrowed-argument-move', 'fn take(a:&[]i32,move b:[]i32){} fn main(){a:[]i32=[42];take(&a,move a)}', 'borrowed by another argument'),
    ('moved-argument-borrow', 'fn take(move a:[]i32,b:&[]i32){} fn main(){a:[]i32=[42];take(move a,&a)}', 'use of moved value'),
    ('ordinary-loop-mutation', 'fn main(){mut a:[]i32=[42];for x in a{a[0]=99}}', 'while iterating'),
    ('ordinary-loop-resize', 'fn main(){mut a:[]i32=[42];for x in a{a.push(1)}}', 'while iterating'),
    ('ordinary-argument-move', 'fn take(a:[]i32,move b:[]i32){} fn main(){a:[]i32=[42];take(a,move a)}', 'borrowed by another argument'),
    ('ordinary-argument-mutation', 'fn edit(a:&mut []i32)->i32{a[0]=99;0} fn read(a:[]i32,n:i32)->i32{a[0]} fn main(){mut a:[]i32=[42];print(read(a,edit(&mut a)))}', 'borrowed by another argument'),
    ('guard-consume', 'enum E{A([]i32),B} fn take(move a:[]i32)->bool{true} fn main(){a:=E.A([42]);match a{.A(v) if take(move v)=>{},.A(v)=>{},.B=>{}}}', 'pattern binding'),
)
STRING_INVALID = (
    ('string-local', 'fn main(){a:="owned"+42.str();b:=a;print(a)}', 'use of moved value'),
    ('string-argument', 'fn take(move a:string){} fn main(){a:="owned"+42.str();take(move a);print(a)}', 'use of moved value'),
    ('string-borrow', 'fn copy(a:&string)->string{a} fn main(){}', 'cannot move out of a borrowed value'),
    ('string-field', 'struct P{a:string} fn main(){a:=P{a:"owned"+42.str()};b:=a.a;print(a.a)}', 'use of moved value'),
    ('string-struct', 'struct P{a:string} fn main(){a:=P{a:"owned"+42.str()};b:=a;print(a.a)}', 'use of moved value'),
    ('string-enum', 'enum E{A(string),B} fn main(){a:=E.A("owned"+42.str());b:=a;c:=a}', 'use of moved value'),
    ('string-clone-moved', 'fn main(){a:="owned"+42.str();b:=a;c:=a.clone()}', 'use of moved value'),
    ('string-branch', 'fn f(flag:bool){a:="owned"+42.str();if flag{b:=a};print(a)} fn main(){f(true)}', 'use of moved value'),
    ('string-backedge', 'fn main(){a:="owned"+42.str();for i in 0..2{b:=a;print(b)}}', 'moved value'),
    ('string-defer', 'fn main(){a:="owned"+42.str();defer{print(a)};b:=a}', 'captured by defer'),
)
TASK_INVALID = (
    ('plain-await', 'fn work(n:i32)->[]i32{[n]} fn main(){task scope j(1){id:=spawn j work(42);a:=await j(id)}}', 'move-only task results require await!'),
    ('fixed-await', 'fn work(n:i32)->[i32;1]{[n]} fn main(){task scope j(1){id:=spawn j work(42);a:=await j(id)}}', 'move-only task results require await!'),
    ('spawn-consumes', 'fn work(move a:[]i32){} fn main(){a:[]i32=[42];task scope j(1){id:=spawn j work(move a)};print(a[0])}', 'use of moved value'),
)


def call(program, args, cwd, status=0, expected=None):
    result = subprocess.run([str(program), *map(str, args)], cwd=cwd,
                            capture_output=True, text=True, timeout=90)
    if result.returncode != status or (expected is not None and result.stdout != expected):
        raise AssertionError(f'{args}: exit {result.returncode}, expected {status}\n'
                             f'{result.stdout!r}\n{result.stderr}')
    return result


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if system == 'Linux' and machine == 'x86_64':
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host {system}/{machine}')


def negative(compiler, source, message, target, work):
    # Public check/build diagnostics must agree across backend/worker modes,
    # while failed publication retains the previous output byte for byte.
    records = []
    modes = ([], ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'])
    workers = ('1', '4') if host_target() == target == TARGETS[0] else ('1',)
    for mode in modes:
        for jobs in workers:
            flags = ['--target', target, '--jobs', jobs, *mode]
            checked = call(compiler, ['check', source, '--agent', *flags], work, 1)
            report = json.loads(checked.stdout)
            assert report['kind'] == 'error' and message in report['message'], report
            assert report['path'] == str(source), report
            assert 0 <= report['start'] <= report['end'] <= len(source.read_bytes()), report
            if source.stem in ('invalid-local', 'invalid-length', 'invalid-string-local', 'invalid-field'):
                # Primary error locations identify the invalid use. The earlier
                # ownership transfer is distinct context, not the error span.
                expected = source.read_bytes().index(b'print(a') + len(b'print(')
                assert report['start'] == expected and report['end'] == expected + 1, report
            records.append(report)
            image = work / 'previous-image'
            image.write_bytes(b'previous image')
            built = call(compiler, ['build', source, *flags, '-o', image], work, 1)
            assert report['message'] in built.stderr and image.read_bytes() == b'previous image'
    assert all(record == records[0] for record in records), records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    builds, errors = 0, 0
    with tempfile.TemporaryDirectory(prefix='dunder-array-moves-') as temporary:
        work = Path(temporary).resolve()
        modes = ([], ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'],
                 ['--scalar-opt=on', '--ownership-opt=off', '--mir=on'])
        for target in TARGETS:
            for mode in modes:
                for workers in ('1', '4') if target == host == TARGETS[0] else ('1',):
                    image = work / f'valid-{builds}'
                    call(compiler, ['build', FIXTURES / 'valid.ddr', '--target', target,
                                    '--jobs', workers, *mode, '-o', image], ROOT)
                    if target == host:
                        call(image, [], work, expected='array moves ok\n')
                    builds += 1
            for name, text, message in INVALID + STRING_INVALID:
                source = work / f'invalid-{name}.ddr'
                source.write_text(text + '\n')
                negative(compiler, source, message, target, work)
                errors += 1
        for target in TARGETS:
            for mode in modes:
                for workers in ('1', '4') if target == host == TARGETS[0] else ('1',):
                    image = work / f'references-{builds}'
                    call(compiler, ['build', FIXTURES / 'references.ddr', '--target', target,
                                    '--jobs', workers, *mode, '-o', image], ROOT)
                    if target == host:
                        call(image, [], work, expected='reference contexts ok\n')
                    builds += 1
        for target in TARGETS:
            image = work / f'strings-{target}'
            call(compiler, ['build', FIXTURES / 'strings.ddr', '--target', target, '-o', image], ROOT)
            if target == host:
                call(image, [], work, expected='string moves ok\n')
            builds += 1
        # Readonly defaults must use the same backing buffer and preserve
        # source availability after repeated calls and loops, without clones.
        for target in TARGETS:
            borrowed = work / f'borrowed-{target}'
            call(compiler, ['build', FIXTURES / 'borrow-default.ddr', '--target', target,
                            '--runtime-profile', '-o', borrowed], ROOT)
            if target == host:
                result = call(borrowed, [], work, expected='borrow defaults ok\n')
                counters = json.loads(result.stderr)['counters']
                assert counters['array_clone_calls'] == counters['record_clone_calls'] == 0, counters
            builds += 1
        project = work / 'cached-project'
        shutil.copytree(FIXTURES, project)
        for target in TARGETS:
            cold, warm = work / f'cold-{target}', work / f'warm-{target}'
            flags = ['--target', target, '--incremental', '--timings=json']
            first = call(compiler, ['build', project / 'valid.ddr', *flags, '-o', cold], project)
            second = call(compiler, ['build', project / 'valid.ddr', *flags, '-o', warm], project)
            before, after = json.loads(first.stderr), json.loads(second.stderr)
            assert before['functions']['reused'] == 0 and after['functions']['reused'] > 0, (before, after)
            assert after['functions']['emitted'] < before['functions']['emitted'], (before, after)
            assert cold.read_bytes() == warm.read_bytes(), target
            if target == host:
                call(warm, [], work, expected='array moves ok\n')
            builds += 2
        # Ownership transfer itself must never deep-clone an array, including
        # through parameter/return/constructor/payload-binding boundaries.
        profile = work / 'profile'
        call(compiler, ['build', FIXTURES / 'profile.ddr', '--runtime-profile', '-o', profile], ROOT)
        result = call(profile, [], work, expected='42\n')
        counters = json.loads(result.stderr)['counters']
        assert counters['array_clone_calls'] == counters['record_clone_calls'] == 0, counters
        assert counters['retain_calls'] == 0, counters
        if host == TARGETS[0]:
            task = work / 'task'
            call(compiler, ['build', FIXTURES / 'task.ddr', '-o', task], ROOT)
            call(task, [], work, expected='task moves ok\n')
            for name, text, message in TASK_INVALID:
                source = work / f'task-invalid-{name}.ddr'
                source.write_text(text + '\n')
                negative(compiler, source, message, host, work)
                errors += 1
    print(f'array moves: {builds} native images, {errors} rejected ownership contracts; '
          'readonly call/loop defaults, move/clone/callback/cache modes and zero implicit array clones passed')


if __name__ == '__main__':
    main()
