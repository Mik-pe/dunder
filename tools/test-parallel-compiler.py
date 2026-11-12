#!/usr/bin/env python3
"""Focused functional acceptance of the isolated parallel compiler.

Compare jobs 1/2/4 images, source bindings, integer work counters, diagnostics
and final-fragment caches. ARM64 macOS images and Wasm are actually executed;
Linux targets are image-only. Timing JSON supplies counters only: durations
are never compared or reported. This is not a latency or OS-overlap test.

An optional --baseline must be a matching serial compiler (its default entry
is used without --jobs). Every run uses a fresh child of --output, retaining
sources, images, command logs and summary.json on failure as well as success.
Session range checks establish structural consistency, not content identity:
the production scope and ordinal/declaration sidecars must preserve job origin.
Cross-session protocol/fault-injection belongs to the separate Session tests.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
WASM_RUNNER = r'''
const assert = require('node:assert/strict');
const fs = require('node:fs');
const module = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const print = x => console.log(x.toString());
let instance;
instance = new WebAssembly.Instance(module, {dunder: {
    print_i32: print, print_i64: print, print_f32: print, print_f64: print,
    print_u32: x => print(x >>> 0), print_u64: x => print(BigInt.asUintN(64, x)),
    print_bool: x => print(Boolean(x)), print_rune: x => print(String.fromCodePoint(x)),
    print_string(p) {
        const memory = instance.exports.memory.buffer;
        const n = new DataView(memory).getUint32(p, true);
        print(new TextDecoder('utf-8', {fatal: true}).decode(new Uint8Array(memory, p + 8, n)));
    },
}});
const result = instance.exports.main();
assert.ok(result === undefined || result === 0, 'main must return unit or status0');
'''
CORE = {
    'arithmetic': ('''fn earlier(n:i64)->i64{var total:i64=0;for i in 0..8{total=total+n+i64(i)};total}
fn later(n:i64)->i64{earlier(n)+1}
fn main(){print(later(2));print(later(5))}
''', '45\n69\n'),
    'owned': ('''struct Box{a:[]i64,n:i64}
fn owned(n:i64)->Box{Box{a:[n,n+1,n+2],n:n}}
fn mutate(a:&mut []i64)->i64{var sum:i64=0;for i in 0..a.len{a[i]=a[i]+1;sum=sum+a[i]};sum}
fn relay(n:i64)->Box{owned(n)}
fn main(){x:=relay(5);var a:=x.a;print(mutate(&mut a));print(x.a[0]);print(a[2])}
''', '21\n5\n8\n'),
    'strings': ('''fn integer(n:i64)->string{n.str()}
fn text(n:i64)->string{"h🙂\\0"+integer(n)}
fn count(t:string)->i64{var sum:i64=0;for r in t.runes(){sum=sum+1};sum}
fn main(){t:=text(42);print(count(t));print(t.bytes().len);print(t=="h🙂\\042")}
''', '5\n8\ntrue\n'),
}
TASK_SOURCE = '''fn worker(x:i64)->[]i64{[x,x+1]}
fn answer()->[]i64{task scope s(2){a:=spawn s worker(41);b:=spawn s worker(43);ignored:=await s(b);return await s(a)}}
fn main(){a:=answer();print(a[0]);print(a[1])}
'''
CACHE_SOURCE = '''fn private(x:i32)->i32{x+INC}
fn first(x:i32)->i32{private(x)+private(x+1)}
fn second(x:i32)->i32{private(x)*2}
fn third(x:i32)->i32{var s:=x;for i in 0..4{s=s+i};s}
fn main(){print(first(1)+second(2)+third(0))}
'''


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Probe:
    def __init__(self, output, compiler, baseline):
        self.output = output
        self.drivers = [('jobs1', compiler, 1), ('jobs2', compiler, 2), ('jobs4', compiler, 4)]
        if baseline is not None:
            self.drivers.append(('baseline', baseline, None))
        self.log = []
        self.images = []
        self.contracts = []
        self.executions = {'arm64-macos': 0, 'wasm32': 0}
        self.identities = {str(p): digest(p) for p in (compiler, baseline) if p is not None}

    def run(self, command, cwd, status=0):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=120)
        entry = {'command': command, 'exit': result.returncode,
                 'stdout': result.stdout, 'stderr': result.stderr}
        self.log.append(entry)
        require(result.returncode == status, entry)
        return result

    def case(self, name, source):
        directory = self.output / name
        directory.mkdir()
        path = directory / 'main.ddr'
        path.write_text(source)
        return path

    @staticmethod
    def command(driver, operation, source, flags=()):
        _, compiler, jobs = driver
        args = [compiler, operation, source]
        if jobs is not None:
            args.append('--jobs=' + str(jobs))
        return args + list(flags)

    def build(self, driver, source, target, image, flags=()):
        args = self.command(driver, 'build', source,
                            ['--target', target, '--timings=json', '-o', image, *flags])
        result = self.run(args, source.parent)
        rows = [json.loads(line) for line in result.stderr.splitlines()
                if line.startswith('{') and '"kind":"timings"' in line]
        require(len(rows) == 1, result.stderr)
        data = rows[0]
        counters = {key: data[key] for key in ('functions', 'optimizer', 'inlining', 'mir')}
        detail = data.get('compiler_detail')
        if detail is not None:
            require(detail['clock_ok'], detail)
            counters['profile'] = {key: detail[key] for key in ('calls', 'work')}
        sha = digest(image)
        self.images.append({'case': source.parent.name, 'target': target, 'driver': driver[0],
                            'path': str(image), 'sha256': sha, 'counters': counters})
        return sha, counters

    def execute(self, image, target, expected):
        if target == 'arm64-macos':
            command = [image]
        elif target == 'wasm32':
            command = ['node', '-e', WASM_RUNNER, image]
        else:
            return  # Linux images are never counted as executed coverage.
        result = self.run(command, image.parent)
        require((result.stdout, result.stderr) == (expected, ''), self.log[-1])
        self.executions[target] += 1

    def matrix(self, source, expected, targets=TARGETS, flags=(), bindings=False):
        for target in targets:
            pairs, reports = [], []
            for driver in self.drivers:
                image = source.parent / (target + '-' + driver[0])
                pairs.append(self.build(driver, source, target, image, flags))
                self.execute(image, target, expected)
                if bindings:
                    result = self.run(self.command(driver, 'check', source,
                                      ['--agent', '--target', target, *flags]), source.parent)
                    report = [json.loads(line) for line in result.stdout.splitlines()]
                    require(report and report[-1].get('kind') == 'ok' and
                            any(row.get('kind') == 'binding' for row in report), report)
                    reports.append((result.stdout, result.stderr))
            require(all(pair == pairs[0] for pair in pairs), (source, target, pairs))
            require(not reports or all(r == reports[0] for r in reports), (source, target, reports))
            self.contracts.append({'case': source.parent.name, 'target': target,
                                   'images_and_integer_counters_equal': True,
                                   'bindings_equal': bindings})

    def diagnostics(self):
        sources = {
            'reachable_error': '''fn first(n:i64)->i64{var s:i64=0;for i in 0..8{s=s+n};s}
fn bad(n:i64)->i64{true}
fn main(){print(first(1));print(bad(2))}
''',
            'unused_error': '''fn first(n:i64)->i64{n+1}
fn bad(n:i64)->i64{true}
fn main(){print(first(41))}
''',
            'module_error': 'import broken as m\nfn main(){print(m.good(1));print(m.bad(2))}\n',
        }
        for name, source in sources.items():
            path = self.case(name, source)
            error_path, error_source = path, source
            if name == 'module_error':
                error_path = path.parent / 'broken.ddr'
                error_source = 'pub fn good(x:i64)->i64{x+1}\npub fn bad(x:i64)->i64{true}\n'
                error_path.write_text(error_source)
            builds, reports = [], []
            for driver in self.drivers:
                image = path.parent / driver[0]
                result = self.run(self.command(driver, 'build', path, ['-o', image]), path.parent, 1)
                require(not image.exists(), ('published a failed build', image))
                require(str(error_path) + ':2:' in result.stderr and 'type mismatch' in result.stderr,
                        self.log[-1])
                builds.append((result.stdout, result.stderr))
                result = self.run(self.command(driver, 'check', path, ['--agent']), path.parent, 1)
                report = json.loads(result.stdout)
                start = error_source.index('true') + len('true')
                require((report['path'], report['start'], report['end']) ==
                        (str(error_path), start, start + 1), report)
                reports.append((result.stdout, result.stderr))
            require(all(x == builds[0] for x in builds), builds)
            require(all(x == reports[0] for x in reports), reports)
            self.contracts.append({'case': name, 'exact_source_diagnostic_equal': True})

    def globals_and_callback(self):
        source = self.case('globals_callback', '')
        oracle = source.parent / 'oracle.c'
        oracle.write_text('int twice(int (*f)(int),int x){return f(x)+f(x+1);}\n')
        library = source.parent / 'oracle.dylib'
        self.run([*shlex.split(os.environ.get('CC', 'cc')), '-dynamiclib', '-fPIC', oracle, '-o', library],
                 source.parent)
        quoted = str(library).replace('\\', '\\\\').replace('"', '\\"')
        source.write_text('capability unsafe\n@extern("' + quoted + '''","twice") unsafe fn twice(f:extern fn(i32)->i32,x:i32)->i32
fn step(x:i32)->i32{var s:=x;for i in 0..2{s=s+1};s}
fn memory(x:i64)->i64{unsafe{p:=static_bytes("native",32);store_i64(p,x);load_i64(p)}}
fn main(){unsafe{print(twice(callback(step),20));print(memory(42));print("literal")}}
''')
        self.matrix(source, '45\n42\nliteral\n', TARGETS[:3])

    def saturation(self):
        source = 'fn step(x:i32)->i32{x+1}\n'
        for function in range(20):
            source += ('fn consume' + str(function) + '()->i32{var sum:=0;' +
                       'sum=sum+step(1);' * 100 + 'sum}\n')
        source += 'fn main(){var total:=0;'
        source += ''.join('total=total+consume' + str(f) + '();' for f in range(20))
        path = self.case('inline_frontier', source + 'print(total)}\n')
        self.matrix(path, '4000\n', ('arm64-macos',))
        counts = self.images[-1]['counters']['inlining']
        require(counts['calls'] > 0 and 61440 <= counts['estimated_words'] <= 65536,
                ('fixture no longer reaches global inline frontier', counts))

    def cache(self):
        path = self.case('cache', CACHE_SOURCE.replace('INC', '1'))
        variants = []
        for driver in self.drivers:
            # Only this run-owned case's cache is removed between driver variants.
            shutil.rmtree(path.parent / 'build', ignore_errors=True)
            path.write_text(CACHE_SOURCE.replace('INC', '1'))
            stages = []
            for phase in ('cold', 'warm', 'mixed', 'edited', 'restored'):
                if phase == 'mixed':
                    removed = []
                    for entry in (path.parent / 'build').rglob('*.fn'):
                        if entry.name in ('1.fn', '3.fn'):
                            entry.unlink()
                            removed.append(entry.name)
                    require(sorted(removed) == ['1.fn', '3.fn'], removed)
                if phase in ('edited', 'restored'):
                    path.write_text(CACHE_SOURCE.replace('INC', '2' if phase == 'edited' else '1'))
                image = path.parent / (driver[0] + '-' + phase)
                pair = self.build(driver, path, 'arm64-macos', image, ['--incremental'])
                self.execute(image, 'arm64-macos', '21\n' if phase == 'edited' else '17\n')
                counters = pair[1]
                if phase == 'cold':
                    require(counters['functions']['reused'] == 0 and counters['mir']['functions'] > 0, counters)
                elif phase == 'warm':
                    require(counters['functions']['reused'] > 0 and counters['mir']['functions'] == 0, counters)
                elif phase == 'mixed':
                    require(counters['functions']['reused'] > 0 and counters['mir']['functions'] > 0, counters)
                stages.append(pair)
            require(stages[0][0] == stages[1][0] == stages[2][0] == stages[4][0], stages)
            require(stages[3][0] != stages[0][0], 'private helper edit did not alter the final image')
            variants.append(stages)
        require(all(x == variants[0] for x in variants), variants)
        self.contracts.append({'case': 'cache', 'cold_warm_mixed_private_edit_equal': True})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='matching serial compiler; invoked without --jobs')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/parallel-compiler-qa')
    args = parser.parse_args()
    if platform.system() != 'Darwin' or platform.machine() not in ('arm64', 'aarch64'):
        parser.error('this native pthread/callback acceptance requires ARM64 macOS')
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix='run-', dir=output))
    probe = Probe(run, compiler, baseline)
    passed = False
    try:
        for name, (source, expected) in CORE.items():
            probe.matrix(probe.case(name, source), expected, bindings=True)
        probe.diagnostics()
        probe.globals_and_callback()
        probe.saturation()
        probe.cache()
        for name, flags in (
            ('mir_off', ['--mir=off']), ('profile_serial', ['--profile-compiler']),
            ('mir_pass_off', ['--mir-opt=off']), ('values_off', ['--mir-values=off']),
            ('scalar_inline_off', ['--scalar-opt=off', '--inline-opt=off']),
        ):
            probe.matrix(probe.case(name, CORE['owned'][0]), CORE['owned'][1], ('arm64-macos',), flags)
        probe.matrix(probe.case('source_tasks', TASK_SOURCE), '41\n42\n', ('arm64-macos',))
        for path, sha in probe.identities.items():
            require(digest(Path(path)) == sha, ('compiler changed during acceptance', path))
        passed = True
    finally:
        summary = {'result': 'passed' if passed else 'failed', 'run': str(run),
                   'compilers_sha256': probe.identities, 'tool_sha256': digest(Path(__file__)),
                   'images': probe.images, 'executions': probe.executions, 'contracts': probe.contracts,
                   'linux_execution': 'not run; cross-target image comparisons only',
                   'dispatch_proof': 'real source tasks exercised; compiler OS-overlap not measured or asserted',
                   'scope': 'functional equality; no latency assertions or timing results'}
        (run / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        (run / 'commands.json').write_text(json.dumps(probe.log, indent=2) + '\n')
        print(('PASS' if passed else 'FAIL') + ' parallel compiler: ' + str(len(probe.images)) +
              ' images, ' + str(sum(probe.executions.values())) + ' native/Wasm executions; ' + str(run))


if __name__ == '__main__':
    main()
