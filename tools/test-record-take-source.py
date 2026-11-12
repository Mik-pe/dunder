#!/usr/bin/env python3
"""Activated source ownership/capacity/cache contracts; no latency assertions.

The comparison compiler must predate record_take frontend activation. Runtime
event counters prove this compiler's source hook removed real aggregate copies;
native/Wasm assertions independently check capacity, snapshots and RC lifetime.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
MODES = ((), ('--scalar-opt=off',), ('--mir=off',), ('--scalar-opt=off', '--mir=off'))
PROFILE = '''struct Leaf { values: []i64 }
struct Item { first: Leaf, second: Leaf, name: string }
fn make() -> Item {
    var values: []i64 = []; values.reserve(64)
    for i in 0..9 { values.push(i64(i)) }
    Item { first: Leaf { values: values }, second: Leaf { values: values }, name: "value" }
}
fn relay() -> Item { make() }
fn main() {
    var value := relay()
    assert(value.first.values.cap() == 9 && value.second.values.cap() == 9)
    snapshot := value; value.first.values[0] = 99
    assert(snapshot.first.values[0] == 0 && value.second.values[0] == 0)
    print(42)
}
'''
CACHE = '''struct Leaf { values: []i64 }
struct Item { leaf: Leaf }
fn make() -> Item { Item { leaf: Leaf { values: [41, 42] } } }
fn main() { value := make(); assert(value.leaf.values.cap() == 8); print(i32(value.leaf.values[1])) }
'''
TRAP_PREFIX = '''struct Item { values: []i64, title: string }
fn failure() -> Item { print(11); assert(false); Item { values: [1], title: "bad" } }
fn later() -> i32 { print(22); z := 0; 1 / z }
fn accept(item: Item, ignored: i32) -> i32 { i32(item.values[0]) + ignored }
'''
TRAPS = (
    (104, 'fn main() { unused := accept(failure(), later()) }'),
    (104, 'fn main() { unused := Item { values: failure().values, title: later().str() } }'),
    (103, 'fn main() { print(11); unused := Item { values: [1], title: "x" }.values[2]; ignored := later() }'),
)


def run(command, *, cwd=ROOT, status=0, output=None):
    result = subprocess.run([str(x) for x in command], cwd=cwd, capture_output=True,
                            text=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', required=True, type=Path)
    args = parser.parse_args()
    compiler, baseline = args.compiler.resolve(strict=True), args.baseline.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    report = {'compiler': hashlib.sha256(compiler.read_bytes()).hexdigest(),
              'baseline': hashlib.sha256(baseline.read_bytes()).hexdigest(),
              'images': 0, 'executions': 0, 'profiles': [], 'cache': []}
    with tempfile.TemporaryDirectory(prefix='dunder-record-source-') as temporary:
        work = Path(temporary)
        values = ROOT / 'tests' / 'fixtures' / 'record-take-values.ddr'
        for mode, flags in enumerate(MODES):
            for target, name in enumerate(TARGETS):
                image = work / f'values-{mode}-{target}'
                run([compiler, 'build', values, '--target', name, *flags, '-o', image])
                run([compiler, 'check', values, '--target', name, *flags])
                report['images'] += 1
                if target == host:
                    run([image], output='42\n'); report['executions'] += 1
                elif target == 3:
                    run(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image, '--repeat'], output='42\n')
                    report['executions'] += 2
            source = work / f'profile-{mode}.ddr'; source.write_text(PROFILE)
            profiles = []
            for label, tool in (('old', baseline), ('new', compiler)):
                image = work / f'profile-{mode}-{label}'
                run([tool, 'build', source, '--target', TARGETS[host], *flags, '--runtime-profile', '-o', image])
                result = run([image], output='42\n')
                profile = json.loads(result.stderr)
                assert profile['kind'] == 'runtime_profile'
                profiles.append(profile['counters'])
            old, new = profiles
            for field in ('allocation_count', 'array_clone_calls', 'array_clone_bytes', 'record_clone_calls', 'record_clone_bytes'):
                assert new[field] < old[field], (mode, field, old, new)
            report['profiles'].append({'mode': mode, 'old': old, 'new': new})
            # ALL source bodies restore. Their helper targets must recreate the
            # transitive demand closure without frontend source re-emission.
            cache_work = work / f'cache-{mode}'; cache_work.mkdir()
            source = cache_work / 'input.ddr'; source.write_text(CACHE)
            cold, warm = cache_work / 'cold', cache_work / 'warm'
            before = run([compiler, 'build', source, *flags, '--inline-opt=off', '--incremental', '--timings=json', '-o', cold], cwd=cache_work)
            after = run([compiler, 'build', source, *flags, '--inline-opt=off', '--incremental', '--timings=json', '-o', warm], cwd=cache_work)
            a, b = json.loads(before.stderr), json.loads(after.stderr)
            assert a['functions']['reused'] == 0 and b['functions']['reused'] == 2 and b['functions']['emitted'] == 0, (mode, a, b)
            assert b['mir']['functions'] == 0 and cold.read_bytes() == warm.read_bytes()
            run([warm], output='42\n')
            report['cache'].append({'mode': mode, 'reused': b['functions']['reused'],
                                    'sha256': hashlib.sha256(warm.read_bytes()).hexdigest()})
        # Actual native C callback entry calls a Dunder function that creates,
        # projects and returns nested owned temporaries. No callback body may
        # emit normalization helpers within a source MIR/cache capture.
        c_source = work / 'callback.c'; library = work / ('callback.dylib' if host == 0 else 'callback.so')
        c_source.write_text('int invoke(int (*f)(int), int value) { return f(value); }\n')
        run(['cc', '-dynamiclib' if host == 0 else '-shared', '-fPIC', c_source, '-o', library])
        callback = work / 'callback.ddr'
        callback.write_text(f'''capability unsafe
@extern("{library}", "invoke") unsafe fn invoke(f: extern fn(i32)->i32, n:i32)->i32
struct Leaf {{ values: []i64 }}
struct Item {{ leaf: Leaf }}
fn make(n:i32)->Item {{ var a:[]i64=[]; a.reserve(64); for i in 0..n{{a.push(i64(i))}}; Item{{leaf:Leaf{{values:a}}}} }}
fn answer(n:i32)->i32{{value:=make(n); projected:=make(n).leaf; assert(value.leaf.values.cap()==n && projected.values.cap()==n); i32(projected.values[n-1])+34}}
fn main(){{unsafe{{assert(invoke(callback(answer),9)==42)}};print(42)}}
''')
        for mode, flags in enumerate(MODES):
            image = work / f'callback-{mode}'
            run([compiler, 'build', callback, *flags, '-o', image]); run([image], output='42\n')
        for index, (status, body) in enumerate(TRAPS):
            source = work / f'trap-{index}.ddr'; source.write_text(TRAP_PREFIX + body)
            for target in (host, 3):
                image = work / f'trap-{index}-{target}'
                run([compiler, 'build', source, '--target', TARGETS[target], '-o', image])
                if target == host:
                    run([image], status=status, output='11\n')
                else:
                    run(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], status=125, output='11\n')
    output = ROOT / 'build' / 'record-take-source'; output.mkdir(parents=True, exist_ok=True)
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print('PASS activated record temporaries: all4 targets, scalar/MIR modes, exact capacities/RC, copy reductions, warm helper demand, native C callback and trap order')


if __name__ == '__main__':
    main()
