#!/usr/bin/env python3
"""Check semantic-only validation and unreachable-body diagnostics on every target."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
COMPOSITE = '''
@c_layout struct Packet { tag: u8, pair: [i32; 2] }
struct Box { text: string, values: []i32 }
enum Action { None, Value(Box) }
fn update(packet: &mut Packet) { packet.tag = 7; packet.pair[0] = 17 }
fn worker() -> i32 {
    var packet := Packet { tag: 1, pair: [1, 2] }
    update(&mut packet)
    var values: []i32 = [1, 2]
    values.push(packet.pair[0])
    fixed: [u8; 4] = [7; 4]
    var single: f32 = 0.5
    single = single + 0.25
    assert(single == 0.75)
    action := Action.Value(Box { text: "å 🌍", values: values })
    match action {
        .None => { return 0 },
        .Value(box) => { return i32(fixed[0]) + box.values[2] },
    }
}
'''
FOREIGN = '''capability unsafe
@extern("probe", "echo") unsafe fn echo(value: i32) -> i32
fn identity(value: i32) -> i32 { value }
fn worker() -> i32 {
    unsafe { callback_value := callback(identity); return echo(42) }
}
'''


def call(compiler, args, directory):
    return subprocess.run([compiler, *args], cwd=directory, env={'PATH': ''},
                          capture_output=True, timeout=30)


def cases():
    for prefix in (COMPOSITE, FOREIGN):
        for main in ('fn main(){print(worker())}', 'fn main(){print(42)}'):
            yield prefix + main, None
    large = ('fn worker()->i32{\n' +
             '\n'.join(f'value{i}:i32={i}' for i in range(768)) +
             '\nvar sum:=value767\n' + '\n'.join('sum=sum+1' for _ in range(1024)))
    yield large + '\nreturn sum\n}\nfn main(){print(42)}', None
    yield large + '\nsmall:u8=256\nreturn sum\n}\nfn main(){print(42)}', 'unsigned integer literal out of range'
    for body, expected in (
        ('fn worker()->i32{true}', 'type mismatch'),
        ('fn worker(){value:=1;value=2}', 'assignment requires a mutable stored value'),
        ('fn pair(a:&mut i32,b:&i32){}\nfn worker(){var x:=0;pair(&mut x,&x)}', 'mutable arguments must not alias'),
        ('@c_layout struct Bad{value:string}\nfn worker(){}', 'C layouts can contain only unmanaged C fields'),
    ):
        for main in (('fn main(){print(worker())}' if 'worker()->i32' in body else 'fn main(){worker()}'), 'fn main(){}'):
            yield body + '\n' + main, expected


def check_case(compiler, source, expected, target, work, baseline):
    result = call(compiler, ['check', str(source), '--agent', '--target', target], work)
    if result.returncode != (1 if expected else 0):
        raise AssertionError(f'check {target}: {result.returncode}\n{result.stdout!r}\n{result.stderr!r}')
    records = [json.loads(line) for line in result.stdout.splitlines()]
    if expected:
        if len(records) != 1 or records[0]['kind'] != 'error' or expected not in records[0]['message']:
            raise AssertionError(f'wrong diagnostic: {records!r}')
        position = records[0]
        if not 0 <= position['start'] <= position['end'] <= source.stat().st_size:
            raise AssertionError(f'invalid source span: {position!r}')
    elif not records or records[-1]['kind'] != 'ok':
        raise AssertionError(f'missing successful check: {records!r}')
    if baseline:
        old = call(baseline, ['check', str(source), '--agent', '--target', target], work)
        if (old.returncode, old.stdout, old.stderr) != (result.returncode, result.stdout, result.stderr):
            raise AssertionError(f'changed check output for {source.name} {target}\n{old.stdout!r}\n{result.stdout!r}')
    image = work / 'image'
    image.write_bytes(b'previous output')
    built = call(compiler, ['build', str(source), '--target', target, '-o', str(image)], work)
    if built.returncode != result.returncode:
        raise AssertionError(f'check/build disagree for {source.name} {target}: {built.stderr!r}')
    if expected:
        if expected.encode() not in built.stderr or image.read_bytes() != b'previous output':
            raise AssertionError(f'build lost diagnostic/output preservation: {built.stderr!r}')
    elif baseline:
        previous = work / 'baseline-image'
        old = call(baseline, ['build', str(source), '--target', target, '-o', str(previous)], work)
        if old.returncode or previous.read_bytes() != image.read_bytes():
            raise AssertionError(f'normal build changed for {source.name} {target}: {old.stderr!r}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='also require identical diagnostics and image bytes')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    count = 0
    with tempfile.TemporaryDirectory(prefix='dunder-check-') as temp:
        work = Path(temp)
        for number, (text, expected) in enumerate(cases()):
            source = work / f'case-{number}.ddr'
            source.write_text(text)
            for target in TARGETS:
                check_case(compiler, source, expected, target, work, baseline)
                count += 1
    print(f'{count} all-target check/build contracts passed')


if __name__ == '__main__':
    main()
