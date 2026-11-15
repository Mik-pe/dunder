#!/usr/bin/env python3
"""Check standard-library owner transfers, explicit snapshots and consumed arguments."""
import argparse
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
IMPORTS = '''import .stdlib.map as map
import .stdlib.set as set
import .stdlib.process as process
import .stdlib.json as json
import .stdlib.net as net
'''
INVALID = (
    ('map-key', 'mut m:=map.new();key:="key"+42.str();map.put(&mut m,move key,1);print(key)'),
    ('set-key', 'mut s:=set.new();key:="key"+42.str();_:=set.insert(&mut s,move key);print(key)'),
    ('command-args', 'mut c:=process.command("echo",[]);args:[]string=["hello"];process.add_args(&mut c,move args);print(args.len)'),
    ('command-stdin', 'mut c:=process.command("cat",[]);data:[]u8=[1,2];process.set_stdin(&mut c,move data);print(data.len)'),
    ('json-input', 'data:[]u8=[b\'{\',b\'}\'];doc:=json.parse_bytes(move data);print(data.len)'),
)
RESOURCE_INVALID = (
    ('stream-set-copy', 's:=net.stream_set();other:=s;print(net.set_len(&s))', 'use of moved value'),
    ('pool-insert', 'fn bad(move s:net.TcpStream){mut pool:=net.stream_set();slot:=net.set_insert(&mut pool,move s);print(net.stream_fd(&s))} fn main(){}', 'use of moved value'),
    ('stream-set-clone', 's:=net.stream_set();other:=s.clone()', 'resource'),
    ('listener-clone', 'fn bad(l:net.TcpListener){other:=l.clone()} fn main(){}', 'resource'),
    ('stream-clone', 'fn bad(s:net.TcpStream){other:=s.clone()} fn main(){}', 'resource'),
    ('child-clone', 'fn bad(c:process.Child){other:=c.clone()} fn main(){}', 'resource'),
    ('child-set-clone', 'fn bad(c:process.ChildSet){other:=c.clone()} fn main(){}', 'resource'),
    ('result-clone', 'fn bad(r:process.SpawnResult){other:=r.clone()} fn main(){}', 'resource'),
)


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return TARGETS[0]
    if host[0] == 'Linux' and host[1] in ('arm64', 'aarch64'):
        return TARGETS[1]
    if host == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported execution host: {host}')


def run(args, *, cwd, expected=0):
    result = subprocess.run(list(map(str, args)), cwd=cwd, capture_output=True, text=True, timeout=90)
    assert result.returncode == expected, (args, result.returncode, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    images = rejected = 0
    modes = ([], ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'])
    with tempfile.TemporaryDirectory(prefix='dunder-stdlib-ownership-') as temporary:
        work = Path(temporary)
        # Import the checked-out sources explicitly, independently of the
        # standard-library snapshot embedded in the compiler under test.
        shutil.copytree(ROOT / 'stdlib', work / 'stdlib')
        for target in TARGETS:
            for mode in modes:
                for fixture, output, clones in (
                    ('stdlib_ownership.ddr', 'stdlib ownership ok\n', 0),
                    ('stdlib_snapshots.ddr', 'stdlib snapshots ok\n', 1),
                    ('stdlib_sort_moves.ddr', 'stdlib sort moves ok\n', 0),
                ):
                    image = work / f'image-{images}'
                    run([compiler, 'build', ROOT / 'tests/fixtures' / fixture,
                         '--target', target, '--jobs', '1', '--runtime-profile', *mode, '-o', image], cwd=ROOT)
                    if target == host:
                        result = run([image], cwd=work)
                        assert result.stdout == output, result.stdout
                        counters = json.loads(result.stderr)['counters']
                        assert counters['array_clone_calls'] == clones, counters
                        assert counters['record_clone_calls'] == 0, counters
                    images += 1
                for name, body, message in (
                    *((name, 'fn main(){' + body + '}', 'use of moved value') for name, body in INVALID),
                    *((name, body if body.startswith('fn ') else 'fn main(){' + body + '}', message)
                      for name, body, message in RESOURCE_INVALID),
                ):
                    source = work / f'{name}.ddr'
                    source.write_text(IMPORTS + body + '\n')
                    checked = run([compiler, 'check', source, '--target', target,
                                   '--jobs', '1', '--agent', *mode], cwd=work, expected=1)
                    diagnostic = json.loads(checked.stdout)
                    assert message in diagnostic['message'], diagnostic
                    assert diagnostic['path'] == str(source), diagnostic
                    image = work / f'invalid-{rejected}'
                    image.write_bytes(b'previous image')
                    built = run([compiler, 'build', source, '--target', target,
                                 '--jobs', '1', *mode, '-o', image], cwd=work, expected=1)
                    assert diagnostic['message'] in built.stderr and image.read_bytes() == b'previous image'
                    rejected += 1
    print(f'stdlib ownership: {images} images, {rejected} rejected consumed arguments; '
          'identity, capacity, explicit snapshots and clone counters passed')


if __name__ == '__main__':
    main()
