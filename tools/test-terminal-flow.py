#!/usr/bin/env python3
"""Check ownership joins after terminal edges without weakening dead-source checks."""
import argparse
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile


TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
MODES = (('default', ()), ('direct', ('--mir=off', '--inline-opt=off',
                                      '--scalar-opt=off', '--ownership-opt=off')))
PRELUDE = '''struct Value { ok: bool, value: string }
fn make() -> Value { Value { ok: true, value: "owned" } }
'''


def program(body):
    return PRELUDE + 'fn work(flag: bool) -> Value {\n' + body + '''\n}
fn main() { answer := work(false); print(answer.value) }
'''


VALID = (
    ('reachable', program('''result := make()
if !result.ok { return result }
print(result.value)
make()'''), 'owned\nowned\n'),
    ('target-return', program('''if !target_is("macos") { return make() }
result := make()
if !result.ok { return result }
print(result.value)
make()'''), {'arm64-macos': 'owned\nowned\n', 'arm64-linux': 'owned\n',
             'x86_64-linux': 'owned\n'}),
    ('unconditional-return', program('''return make()
result := make()
if !result.ok { return result }
print(result.value)
make()'''), 'owned\n'),
    ('dead-else-return', program('''return make()
result := make()
if flag { print(result.value) } else { return result }
print(result.value)
make()'''), 'owned\n'),
    ('dead-nested-branches', program('''return make()
result := make()
if flag {
    if !result.ok { return result }
    print(result.value)
} else if !result.ok { return result }
print(result.value)
make()'''), 'owned\n'),
    ('dead-match', 'enum Choice { A, B, C }\n' + program('''return make()
result := make()
match Choice.B {
    .A => { return result },
    .B => { print(result.value) },
    .C => { if flag { return result }; print(result.value) }
}
print(result.value)
make()'''), 'owned\n'),
    ('dead-break', program('''result := make()
for i in 0..1 {
    break
    if flag { moved := result; print(moved.value); break }
    print(result.value)
}
print(result.value)
result'''), 'owned\nowned\n'),
    ('dead-continue', program('''result := make()
for i in 0..1 {
    continue
    if flag { moved := result; print(moved.value); continue }
    print(result.value)
}
print(result.value)
result'''), 'owned\nowned\n'),
    ('dead-match-loop-exits', 'enum Choice { A, B, C }\n' + program('''result := make()
for i in 0..1 {
    break
    match Choice.B {
        .A => { moved := result; print(moved.value); continue },
        .B => { print(result.value) },
        .C => { break }
    }
    print(result.value)
}
result'''), 'owned\n'),
    ('partial-return', '''struct Pair { left: string, right: string }
fn work(flag: bool) -> string {
    pair := Pair { left: "left", right: "right" }
    if flag { return pair.left }
    print(pair.left)
    pair.right
}
fn main() { print(work(false)) }
''', 'left\nright\n'),
    ('defer-return', program('''message := "deferred"
defer { print(message) }
if flag { return make() }
result := make()
if !result.ok { return result }
print(result.value)
result'''), 'owned\ndeferred\nowned\n'),
)

INVALID = (
    ('dead-type', program('return make()\nwrong: i32 = true\nmake()'), 'type mismatch'),
    ('dead-name', program('return make()\nmissing_name()\nmake()'), 'unknown'),
    ('dead-straight-move', program('''return make()
result := make(); moved := result; print(moved.value)
print(result.value)
make()'''), 'moved value'),
    ('dead-branch-move', program('''return make()
result := make()
if flag { moved := result; print(moved.value) }
print(result.value)
make()'''), 'moved value'),
    ('dead-value-branch-move', program('''return make()
result := make()
selected := if flag { result } else if result.ok { make() } else { make() }
print(selected.value); print(result.value)
make()'''), 'moved value'),
    ('live-branch-move', program('''result := make()
if flag { moved := result; print(moved.value) }
print(result.value)
make()'''), 'moved value'),
    ('live-match-move', 'enum Choice { A, B }\n' + program('''result := make()
match Choice.B { .A => { moved := result; print(moved.value) }, .B => {} }
print(result.value)
make()'''), 'moved value'),
    ('partial-branch-move', '''struct Pair { left: string, right: string }
fn bad(flag: bool) { pair := Pair { left: "left", right: "right" }
    if flag { moved := pair.left; print(moved) }; print(pair.left) }
fn main() { bad(false) }
''', 'moved value'),
    ('loop-backedge', 'fn main() { value := "x"; for i in 0..2 { moved := value; print(moved) } }',
     'reinitialized'),
    ('loop-continue', 'fn main() { value := "x"; for i in 0..2 { moved := value; print(moved); continue } }',
     'reinitialized'),
    ('loop-break-move', 'fn main() { value := "x"; for i in 0..2 { moved := value; print(moved); break }; print(value) }',
     'moved value'),
    ('active-borrow', program('''result := make(); loan := &result
moved := result; print(moved.value); print(loan.value)
make()'''), 'borrow'),
    ('defer-capture', program('result := make()\ndefer { print(result.value) }\nreturn result'), 'defer'),
    ('break-is-not-function-return', '''fn bad(flag: bool) -> i32 {
    while flag { if flag { break } else { continue } }
}
fn main() { print(bad(false)) }
''', 'without returning'),
)


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return TARGETS[0]
    if host[0] == 'Linux' and host[1] in ('arm64', 'aarch64'):
        return TARGETS[1]
    if host == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported native execution host: {host}')


def invoke(argv, cwd, status=0):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, capture_output=True,
                            text=True, timeout=90)
    assert result.returncode == status, (argv, result.returncode, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    runners = {host_target(): []}
    for value in args.runner:
        target, separator, command = value.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    checks = images = executions = rejections = cache_checks = 0
    with tempfile.TemporaryDirectory(prefix='dunder-terminal-flow-') as temporary:
        work = Path(temporary).resolve()
        for name, text, expected in VALID:
            source = work / (name + '.ddr'); source.write_text(text)
            for target in TARGETS:
                for mode, flags in MODES:
                    result = invoke([compiler, 'check', source, '--agent', '--target', target, *flags], work)
                    assert json.loads(result.stdout.splitlines()[-1])['kind'] == 'ok', result
                    checks += 1
                    previous = None
                    for workers in (1, 4):
                        image = work / f'{name}-{target}-{mode}-{workers}'
                        invoke([compiler, 'build', source, '--target', target, '--jobs', workers,
                                *flags, '-o', image], work)
                        data = image.read_bytes()
                        assert data[:4] == (b'\xcf\xfa\xed\xfe' if target == TARGETS[0] else b'\x7fELF')
                        if previous is not None:
                            assert data == previous, (name, target, mode, 'worker images differ')
                        previous = data; images += 1
                        if workers == 1 and target in runners:
                            ran = invoke([*runners[target], image], work)
                            output = expected[target] if isinstance(expected, dict) else expected
                            assert (ran.stdout, ran.stderr) == (output, ''), (name, target, mode, ran)
                            executions += 1
                    if name in ('target-return', 'unconditional-return', 'dead-match') and mode == 'default':
                        image = work / f'{name}-{target}-cached'
                        for repeat in range(2):
                            invoke([compiler, 'build', source, '--target', target, '--jobs', '4',
                                    '--incremental', '-o', image], work)
                            assert image.read_bytes() == previous, (name, target, 'cache image differs')
                            cache_checks += 1
        for name, text, message in INVALID:
            source = work / (name + '.ddr'); source.write_text(text + '\n')
            for target in TARGETS:
                for mode, flags in MODES:
                    result = invoke([compiler, 'check', source, '--agent', '--target', target, *flags], work, 1)
                    report = json.loads(result.stdout.splitlines()[-1])
                    assert report['kind'] == 'error' and message in report['message'].lower(), (name, report)
                    image = work / f'rejected-{target}-{mode}'; image.write_bytes(b'previous image')
                    built = invoke([compiler, 'build', source, '--target', target, '--jobs', '4',
                                    *flags, '-o', image], work, 1)
                    assert message in built.stderr.lower() and image.read_bytes() == b'previous image', (name, built)
                    rejections += 1
    print(f'Terminal ownership flow: {checks} checks, {images} native images, {executions} executions, '
          f'{cache_checks} cache comparisons, {rejections} diagnostic contracts passed')


if __name__ == '__main__':
    main()
