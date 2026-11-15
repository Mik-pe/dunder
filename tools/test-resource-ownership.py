#!/usr/bin/env python3
"""Check unique OS-handle ownership without duplicating descriptors or processes."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
PREFIX = 'capability network, process\nimport std.net as net\nimport std.process as process\n'

# These were accepted when a resource's integer fd/pid fields made it copyable,
# or when clone recursively copied those integers into another apparent owner.
INVALID = (
    ('listener-move', 'fn main(){opened:=net.listen(net.loopback(),0,4);a:=opened.listener;b:=a;print(net.listener_fd(&a))}', 'moved value'),
    ('stream-move', 'fn f(move a:net.TcpStream){b:=a;print(net.stream_fd(&a))} fn main(){}', 'moved value'),
    ('child-move', 'fn f(move a:process.Child){b:=a;print(process.child_pid(&a))} fn main(){}', 'moved value'),
    ('listener-clone', 'fn f(a:net.TcpListener){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('stream-clone', 'fn f(a:net.TcpStream){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('child-clone', 'fn f(a:process.Child){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('stream-set-clone', 'fn f(a:net.StreamSet){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('child-set-clone', 'fn f(a:process.ChildSet){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('record-clone', 'struct Owner{s:net.TcpStream} fn f(a:Owner){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('enum-clone', 'enum Owner{Live(net.TcpListener),Empty} fn f(a:Owner){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('nested-record-clone', 'struct Inner{s:net.StreamSet} struct Outer{i:Inner} fn f(a:Outer){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('nested-enum-clone', 'struct Inner{c:process.ChildSet} enum Outer{Live(Inner),Empty} fn f(a:Outer){b:=a.clone()} fn main(){}', 'resource values cannot be cloned'),
    ('borrowed-return', 'fn f(a:net.TcpListener)->net.TcpListener{a} fn main(){}', 'borrowed'),
    ('borrowed-store', 'fn f(a:process.Child){b:=a} fn main(){}', 'borrowed'),
    ('constructor-move', 'struct Owner{s:net.TcpStream} fn f(move a:net.TcpStream){b:=Owner{s:a};print(net.stream_fd(&a))} fn main(){}', 'moved value'),
    ('call-move', 'fn take(move a:process.Child){} fn f(move a:process.Child){take(move a);print(process.child_pid(&a))} fn main(){}', 'moved value'),
    ('task-input-borrow', '@resource struct R{fd:i32} fn work(r:R)->i32{r.fd} fn main(){r:=R{fd:42};task scope j(1){id:=spawn j work(r);print(await j(id))}}', 'isolated Data'),
    ('task-input-move', '@resource struct R{fd:i32} fn work(move r:R)->i32{r.fd} fn main(){r:=R{fd:42};task scope j(1){id:=spawn j work(move r);print(await j(id))}}', 'isolated Data'),
    ('task-result', '@resource struct R{fd:i32} fn work(n:i32)->R{R{fd:n}} fn main(){task scope j(1){id:=spawn j work(42);r:=await! j(id)}}', 'isolated Data'),
    ('task-world', '@resource struct R{fd:i32} struct W{r:R} fn work(n:i32,w:&W)->i32{w.r.fd+n} fn main(){world:=W{r:R{fd:1}};task scope j(1,shared world){id:=spawn j work(41);print(await j(id))}}', 'isolated Data'),
    ('task-workspace', '@resource struct R{fd:i32} fn init()->R{R{fd:42}} fn work(n:i32,w:&mut R)->i32{w.fd+n} fn main(){task scope j(1,init){id:=spawn j work(0);print(await j(id))}}', 'isolated storage'),
)


def call(command, cwd, status=0):
    result = subprocess.run([str(value) for value in command], cwd=cwd,
                            capture_output=True, text=True, timeout=90)
    if result.returncode != status:
        raise AssertionError(f'{command}: exit {result.returncode}, expected {status}\n'
                             f'{result.stdout}\n{result.stderr}')
    return result


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return TARGETS[0]
    if host[0] == 'Linux' and host[1] in ('arm64', 'aarch64'):
        return TARGETS[1]
    if host == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise ValueError(f'unsupported execution host {host}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--jobs', type=int, choices=(1, 4), default=1)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    modes = ([], ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'])
    checked, executed = 0, 0
    with tempfile.TemporaryDirectory(prefix='dunder-resource-ownership-') as temporary:
        work = Path(temporary)
        for target in TARGETS:
            jobs = str(args.jobs if host == target == TARGETS[0] else 1)
            for name, body, expected in INVALID:
                source = work / (name + '.ddr')
                source.write_text(PREFIX + body + '\n')
                for mode in modes:
                    flags = ['--target', target, '--jobs', jobs, *mode]
                    result = call([compiler, 'check', source, '--agent', *flags], work, 1)
                    diagnostic = json.loads(result.stdout)
                    assert diagnostic['kind'] == 'error' and expected in diagnostic['message'], diagnostic
                    assert diagnostic['path'] == str(source), diagnostic
                    assert 0 <= diagnostic['start'] < diagnostic['end'] <= len(source.read_bytes()), diagnostic
                    image = work / 'preserved-image'
                    image.write_bytes(b'previous valid image')
                    built = call([compiler, 'build', source, *flags, '-o', image], work, 1)
                    assert diagnostic['message'] in built.stderr, (diagnostic, built.stderr)
                    assert image.read_bytes() == b'previous valid image'
                    checked += 1
            for fixture, expected_output in (
                    ('resource-ownership', 'resource handles ok\n'),
                    ('resource-transfers', 'resource transfers ok\n')):
                source = ROOT / 'tests/fixtures' / (fixture + '.ddr')
                call([compiler, 'check', source, '--target', target, '--jobs', jobs], ROOT)
                for index, mode in enumerate(modes):
                    image = work / f'{fixture}-{target}-{index}'
                    call([compiler, 'build', source, '--target', target,
                          '--runtime-profile', '--jobs', jobs, *mode, '-o', image], ROOT)
                    if target == host:
                        run = call([image], work)
                        assert run.stdout == expected_output, run.stdout
                        counters = json.loads(run.stderr)['counters']
                        assert counters['array_clone_calls'] == 0, counters
                        # OS APIs also return copyable POD status records. The
                        # pure transfer oracle isolates resource copies from
                        # those legal value copies and must perform none.
                        if fixture == 'resource-transfers':
                            assert counters['record_clone_calls'] == 0, counters
                        executed += 1
    print(f'PASS resource ownership: {checked} diagnostic/build pairs, {executed} host executions')


if __name__ == '__main__':
    main()
