#!/usr/bin/env python3
"""Check nonduplicable resources, inline C transfers and clone-free array swaps."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
VALID = '''capability unsafe
@resource struct Handle { identity: i32 }
struct Envelope { handle: Handle, name: string }
@c_layout struct Inline { values: [i32; 2] }
fn identity(value: Handle) -> i32 { value.identity }
fn forward(move value: Handle) -> Handle { value }
fn flip(values: &mut []string) { values.swap(0, 1) }
fn flip_fixed(values: &mut [i32; 2]) { values.swap(0, 1) }
fn next(value: &mut i32) -> i32 { old := value; value += 1; old }
fn reset(values: &mut []i32) -> i32 { values = [11, 22]; 1 }
fn main() {
    handle := Handle { identity: 42 }
    assert(identity(handle) == 42 && identity(handle) == 42)
    envelope := Envelope { handle: handle, name: "owner" }
    transferred := envelope.handle
    final := forward(move transferred)
    assert(identity(final) == 42)
    mut words: []string = ["left" + 1.str(), "right" + 2.str()]
    unsafe {
        before := addr(&words[0])
        left := load_ptr(addr(&words[0])); right := load_ptr(addr(&words[1]))
        flip(&mut words)
        assert(addr(&words[0]) == before)
        assert(load_ptr(addr(&words[0])) == right && load_ptr(addr(&words[1])) == left)
    }
    assert(words[0] == "right2" && words[1] == "left1")
    words.swap(1, 1)
    assert(words[1] == "left1")
    mut signed: []i32 = [-2147483648, 2147483647]
    signed.swap(0, 1); assert(signed[0] == 2147483647 && signed[1] == -2147483648)
    mut small: []u8 = [0, 255]
    small.swap(0, 1); assert(small[0] == 255 && small[1] == 0)
    mut floats: []f64 = [1.25, -3.5]
    floats.swap(0, 1); assert(floats[0] == -3.5 && floats[1] == 1.25)
    mut fixed: [i32; 2] = [20, 22]
    flip_fixed(&mut fixed); assert(fixed[0] == 22 && fixed[1] == 20)
    mut order := 0
    fixed.swap(next(&mut order), next(&mut order))
    assert(order == 2 && fixed[0] == 20 && fixed[1] == 22)
    mut replaced: []i32 = [1, 2, 3]
    replaced.swap(0, reset(&mut replaced))
    assert(replaced.len == 2 && replaced[0] == 22 && replaced[1] == 11)
    for turn in 0..128 {
        mut source: [i32; 2] = [20, 22]
        mut value := Inline { values: source }
        source = [1, 2]
        assert(value.values[0] == 20 && source[0] == 1)
        value.values.swap(0, 1)
        assert(value.values[0] == 22 && value.values[1] == 20)
        value.values = source
        assert(value.values[0] == 1 && value.values[1] == 2)
    }
    print("resource transfers and swaps ok")
}
'''
INVALID = (
    ('inline-constructor', '@c_layout struct C{a:[i32;2]} fn main(){a:=[20,22];c:=C{a:a};print(a[0])}', 'use of moved value'),
    ('inline-assignment', '@c_layout struct C{a:[i32;2]} fn main(){a:=[20,22];mut c:=C{a:[1,2]};c.a=a;print(a[0])}', 'use of moved value'),
    ('inline-borrow-constructor', '@c_layout struct C{a:[i32;2]} fn bad(a:[i32;2])->C{C{a:a}} fn main(){}', 'cannot move out of a borrowed value'),
    ('inline-borrow-assignment', '@c_layout struct C{a:[i32;2]} fn bad(a:[i32;2],c:&mut C){c.a=a} fn main(){}', 'cannot move out of a borrowed value'),
    ('resource-move', '@resource struct R{fd:i32} fn main(){a:=R{fd:42};b:=a;print(a.fd)}', 'use of moved value'),
    ('resource-borrow', '@resource struct R{fd:i32} fn bad(r:R)->R{r} fn main(){}', 'cannot move out of a borrowed value'),
    ('resource-clone', '@resource struct R{fd:i32} fn main(){a:=R{fd:42};b:=a.clone()}', 'resource values cannot be cloned'),
    ('nested-clone', '@resource struct R{fd:i32} struct W{r:R} fn main(){a:=W{r:R{fd:42}};b:=a.clone()}', 'resource values cannot be cloned'),
    ('enum-clone', '@resource struct R{fd:i32} enum W{R(R),Empty} fn main(){a:=W.R(R{fd:42});b:=a.clone()}', 'resource values cannot be cloned'),
    ('resource-attribute', '@resource fn f(){} fn main(){}', 'invalid declaration attribute'),
    ('swap-immutable', 'fn main(){a:[]i32=[1,2];a.swap(0,1)}', 'swap requires mut'),
    ('swap-readonly', 'fn bad(a:[]string){a.swap(0,1)} fn main(){}', 'swap requires mut'),
    ('swap-loop', 'fn main(){mut a:[]string=["a","b"];for x in a{a.swap(0,1)}}', 'while iterating'),
    ('swap-moved', 'fn main(){mut a:[]string=["a","b"];b:=a;a.swap(0,1)}', 'use of moved value'),
    ('swap-index-type', 'fn main(){mut a:[]i32=[1,2];a.swap(true,1)}', 'type mismatch'),
)
TASK_INVALID = (
    ('task-input', '@resource struct R{fd:i32} fn work(r:R)->i32{r.fd} fn main(){r:=R{fd:42};task scope s(1){key:=spawn s work(r)}}', 'task worker input must be isolated Data'),
    ('task-result', '@resource struct R{fd:i32} fn work(n:i32)->R{R{fd:n}} fn main(){task scope s(1){key:=spawn s work(42);r:=await! s(key)}}', 'task worker result must be isolated Data'),
)


def run(command, cwd=ROOT, status=0):
    result = subprocess.run(list(map(str, command)), cwd=cwd, text=True, capture_output=True, timeout=90)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--include-wasm', action='store_true', help='also cross-build the deferred Wasm target')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 'arm64-macos' if platform.system() == 'Darwin' else ('arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
    images = errors = 0
    with tempfile.TemporaryDirectory(prefix='dunder-resource-moves-') as directory:
        work = Path(directory)
        source = work / 'valid.ddr'; source.write_text(VALID)
        modes = ([], ['--scalar-opt=off', '--ownership-opt=off', '--mir=off'])
        for target in TARGETS + (('wasm32',) if args.include_wasm else ()):
            for mode in modes:
                for jobs in (1, 4) if target == host == 'arm64-macos' else (1,):
                    image = work / f'valid-{images}'
                    run([compiler, 'build', source, '--target', target, '--jobs', jobs, *(['--runtime-profile'] if target != 'wasm32' else []), *mode, '-o', image])
                    if target == host:
                        result = run([image], work)
                        assert result.stdout == 'resource transfers and swaps ok\n', result.stdout
                        counters = json.loads(result.stderr)['counters']
                        assert counters['array_clone_calls'] == counters['record_clone_calls'] == 0, counters
                        if not mode: assert counters['retain_calls'] == 0, counters
                    images += 1
            for name, text, message in INVALID:
                invalid = work / (name + '.ddr'); invalid.write_text(text + '\n')
                diagnostics = []
                for mode in modes:
                    checked = run([compiler, 'check', invalid, '--target', target, '--agent', *mode], status=1)
                    record = json.loads(checked.stdout)
                    assert message in record['message'], record
                    diagnostics.append(record)
                    image = work / 'previous'; image.write_bytes(b'previous')
                    built = run([compiler, 'build', invalid, '--target', target, *mode, '-o', image], status=1)
                    assert record['message'] in built.stderr and image.read_bytes() == b'previous', built.stderr
                assert diagnostics[0] == diagnostics[1], diagnostics
                errors += 1
        if host == 'arm64-macos':
            for name, text, message in TASK_INVALID:
                invalid = work / (name + '.ddr'); invalid.write_text(text + '\n')
                result = run([compiler, 'check', invalid], status=1)
                assert message in result.stderr, result.stderr
                errors += 1
        for index in (-1, 2):
            invalid = work / f'bounds-{index}.ddr'
            invalid.write_text(f'fn index(n:i32)->i32{{print(n);n}} fn main(){{mut a:[]string=["a","b"];a.swap(index(0),index({index}));print("unreachable")}}\n')
            image = work / f'bounds-{index}'
            run([compiler, 'build', invalid, '-o', image])
            result = run([image], work, status=103)
            assert result.stdout == f'0\n{index}\n', result.stdout
    print(f'resource moves: {images} images and {errors} rejected contracts; clone-free swaps, C transfers, task isolation and bounds passed')


if __name__ == '__main__':
    main()
