#!/usr/bin/env python3
"""Frozen public compiler-API consumer tests for bounded scalar inlining.

Runs on/off images on the native host, builds every target, preserves
source binding/diagnostic reports, and checks explicit CLI-off and optional
older-baseline byte identity. Native trap codes and pre-trap stdout are exact.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
ENV = {'PATH': '/no-external-tools'}
COUNTERS = ('modeled', 'optimized', 'emitted', 'reused', 'fallbacks')
INLINE = ('entries', 'nodes', 'tokens', 'units', 'calls', 'words', 'estimate')


def frozen_cases():
    valid={
    'eager-unused':('fn keep(a:i32,b:i32)->i32{a} fn effect(x:i32)->i32{print(x);x} fn main(){print(keep(effect(11),effect(22)))}','11\n22\n11\n',0),
    'eager-repeated':('fn twice(a:i32,b:i32)->i32{a+a+b} fn effect(x:i32)->i32{print(x);x} fn main(){print(twice(effect(11),effect(22)))}','11\n22\n44\n',0),
    'short-circuit':('fn safe(a:i32,b:i32)->bool{a==0||b/a>2} fn main(){assert(safe(0,9));assert(safe(2,9));print(42)}','42\n',0),
    'arity-eight':('fn mix(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32)->i32{h+a+d+f+c+b+e+g} fn main(){print(mix(1,2,3,4,5,6,7,8))}','36\n',0),
    'i64':('fn calc(a:i64,b:i64)->i64{(a*7+b)/3} fn main(){value:=calc(1000000000000,17);assert(value==2333333333339);print(42)}','42\n',0),
    'i32-canonical':('fn same(x:i32)->i32{x} fn main(){print(same(-1))}','-1\n',0),
    'precedence':('fn calc(x:i32)->i32{(x*7+3)%5} fn main(){print(20+calc(11)*3)}','20\n',0),
    'nested-calls':('fn inner(x:i32)->i32{x*3} fn outer(x:i32,y:i32)->i32{x+y} fn main(){print(outer(inner(3),inner(4)))}','21\n',0),
    'shadowed-pointer':('fn chosen(x:i32)->i32{x+1} fn other(x:i32)->i32{x+10} fn main(){chosen:=other;print(chosen(3))}','13\n',0),
    'local-fallback':('fn calc(x:i32)->i32{var y:=x;y=y+1;y} fn main(){print(calc(3))}','4\n',0),
    'call-fallback':('fn inc(x:i32)->i32{x+1} fn calc(x:i32)->i32{inc(x)} fn main(){print(calc(3))}','4\n',0),
    'loop-fallback':('fn calc(x:i32)->i32{var sum:=0;for i in 0..x{sum=sum+i};sum} fn main(){print(calc(4))}','6\n',0),
    'arg-order-trap':('fn first(a:i32,b:i32)->i32{a} fn marker()->i32{print(71);4} fn zero()->i32{0} fn main(){print(first(marker(),8/zero()))}','71\n',102),
    'unused-trap':('fn first(a:i32,b:i32)->i32{a} fn zero()->i32{0} fn main(){print(first(3,8/zero()))}','',102),
    'body-trap-order':('fn div(a:i32,b:i32)->i32{a/b} fn marker()->i32{print(72);0} fn main(){print(div(4,marker()))}','72\n',102),
    'overflow-i32':('fn add(a:i32,b:i32)->i32{a+b} fn main(){print(add(2147483647,1))}','',101),
    'overflow-i64':('fn add(a:i64,b:i64)->i64{a+b} fn main(){value:=add(9223372036854775807,1);assert(value==0)}','',101),
    'min-rem':('fn rem(a:i64)->i64{a%(-1)} fn main(){value:=rem(-9223372036854775808);assert(value==0)}','',101),
    'dead-branch-short':('fn safe(a:i32,b:i32)->bool{a!=0&&b/a>2} fn main(){assert(!safe(0,9));print(42)}','42\n',0),
    }
    # Caller/global growth budgets must fall back while preserving all effects.
    valid['caller-budget']=('fn inc(x:i32)->i32{x+1} fn main(){var sum:=0;'+''.join('sum=sum+inc(1);' for _ in range(160))+'print(sum)}','320\n',0)
    functions='fn inc(x:i32)->i32{x+1} '+''.join('fn f'+str(i)+'()->i32{var sum:=0;'+''.join('sum=sum+inc(1);' for _ in range(100))+'sum} ' for i in range(20))
    valid['global-budget']=(functions+'fn main(){var sum:=0;'+''.join(f'sum=sum+f{i}();' for i in range(20))+'print(sum)}','4000\n',0)
    # Producer token scan exhaustion on late structural rejection; a later useful
    # candidate must fall back, while ordinary source diagnostics remain complete.
    large='fn id(x:i32)->i32{x} '+''.join(f'fn reject{i}(x:i32)->i32{{return '+''.join('x+' for _ in range(50))+'x;var y:=x} ' for i in range(340))
    valid['source-budget']=(large+'fn tail(x:i32)->i32{x+1} fn main(){print(tail(9))}','10\n',0)
    large_loans='fn area(){region frame{}} '+''.join(f'fn large{i}(x:i32)->i32{{var y:=x;'+''.join('y=y+1;' for _ in range(40))+'y} ' for i in range(180))
    valid['large-loan-active']=(large_loans+'fn tail(x:i32)->i32{x+1} fn main(){print(tail(9))}','10\n',0)
    invalid=[
    'fn add(x:i32)->i32{x+2147483648} fn main(){print(add(1))}',
    'fn add(x:i32)->i32{x=4;x} fn main(){print(add(1))}',
    'fn add(x:i32)->i32{missing+x} fn main(){}',
    'fn add(x:i32)->i32{if false{return true};x} fn main(){}',
    'fn add(x:i32)->i32{x+1} fn main(){print(add(true))}',
    'fn add(x:i32)->i32{x+1} fn main(){print(add())}',
    'fn add(x:i32)->i32{x+1} fn main(){print(add(1,2))}',
    'fn add(x:i32,x:i32)->i32{x} fn main(){}',
    'fn add(x:i64)->bool{10/x>0} fn main(){}',
    'fn add(x:i32)->i32{if x>0{return x}} fn main(){}',
    ]
    return valid, invalid


def decode(output):
    lines = output.splitlines()
    assert lines[0] == 'COUNTERS' and lines[6] == 'BINDINGS', output
    counters = dict(zip(COUNTERS, map(int, lines[1:6])))
    assert all(value >= 0 for value in counters.values()), counters
    count = int(lines[7]); end = 8 + count * 3
    assert count >= 0 and lines[end] == 'INLINE' and len(lines) == end + 8, output
    bindings = [(int(lines[i]), lines[i+1], lines[i+2]) for i in range(8, end, 3)]
    metrics = dict(zip(INLINE, map(int, lines[end+1:])))
    assert 0 <= metrics['entries'] <= 256 and 0 <= metrics['nodes'] <= 4096, metrics
    assert 0 <= metrics['tokens'] <= 32768 and 0 <= metrics['units'] <= 65536, metrics
    assert 0 <= metrics['calls'] and 0 <= metrics['words'] <= metrics['estimate'] <= 65536, metrics
    return dict(bindings=bindings, counters=counters, inline=metrics)


def fixture_source(source_root, work):
    source = (ROOT / 'tests/fixtures/inline-producer.ddr').read_text()
    for name, alias in (('compile', 'compiler'), ('lexer', 'lexer'), ('macho', 'macho'), ('elf', 'elf')):
        original = 'import ...selfhost.' + name + (' as compiler' if name == 'compile' else '')
        assert source.count(original) == 1, ('fixture import changed', original)
        relative = os.path.relpath(source_root / 'selfhost' / name, work).replace(os.sep, '/')
        source = source.replace(original, 'import ' + json.dumps(relative) + ' as ' + alias)
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='optional older compiler; receives no new inline flag')
    parser.add_argument('--source-root', type=Path, default=ROOT, help='compiler API checkout imported by the fixture')
    parser.add_argument('--report', type=Path, help='optional JSON with hashes, counters and raw command output')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    source_root = args.source_root.resolve(strict=True)
    if platform.system() == 'Darwin' and platform.machine() == 'arm64':
        host = 0
    elif platform.system() == 'Linux' and platform.machine() in ('arm64', 'aarch64', 'x86_64'):
        host = 1 if platform.machine() in ('arm64', 'aarch64') else 2
    else:
        parser.error('execution requires ARM64 macOS or ARM64/x86-64 Linux')
    inputs = {compiler, Path(__file__).resolve(), ROOT / 'tests/fixtures/inline-producer.ddr',
              *source_root.glob('selfhost/*.ddr')}
    if baseline:
        inputs.add(baseline)
    hashes = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    valid, invalid = frozen_cases()
    assert len(valid) == 23 and len(invalid) == 10
    evidence = dict(complete=False, hashes=hashes, host=platform.uname()._asdict(),
                    valid_source_hashes={name: hashlib.sha256(source.encode()).hexdigest()
                                         for name, (source, _, _) in valid.items()}, images=[], commands=[],
                    invalid_source_hashes=[hashlib.sha256(source.encode()).hexdigest() for source in invalid],
                    expected_results={name: dict(stdout=stdout, native_status=status)
                                      for name, (_, stdout, status) in valid.items()},
                    limitations=['Linux targets are cross-built when not the native host',
                                 'cache counters describe performed work; source bindings are compared separately'])
    builds = diagnostics = executions = cross_builds = cli_checks = 0
    try:
        with tempfile.TemporaryDirectory(prefix='dunder-inline-producer-') as directory:
            work = Path(directory).resolve()

            def call(command, expected=0):
                result = subprocess.run([str(value) for value in command], cwd=work, env=ENV,
                                        capture_output=True, text=True, timeout=120)
                if args.report:
                    evidence['commands'].append(dict(command=[str(x) for x in command], status=result.returncode, expected_status=expected,
                                                     stdout=result.stdout, stderr=result.stderr))
                assert result.returncode == expected, (command, result.returncode, expected, result.stdout, result.stderr)
                return result

            fixture = work / 'inline-producer.ddr'; fixture.write_text(fixture_source(source_root, work))
            runner = work / 'consumer'
            # The selected candidate's CLI default is deliberately irrelevant.
            call([compiler, 'build', fixture, '-o', runner, '--inline-opt=off'])

            def build(name, source, target, mode, scalar='on', cache='', check=False, expected=0):
                nonlocal builds, diagnostics
                path = work / (name + '.ddr'); path.write_text(source)
                image = work / f'{name}-{target}-{mode}-{scalar}'
                sentinel = b'previous successful output'
                if expected or check:
                    image.write_bytes(sentinel)
                result = call([runner, path, image, target, mode, scalar, cache,
                               'check' if check else 'build'], expected)
                assert result.stderr == '', (name, target, mode, result.stderr)
                if expected or check:
                    assert image.read_bytes() == sentinel, (name, target, 'failed/check call replaced image')
                if expected:
                    assert result.stdout.startswith('ERROR\n'), result.stdout
                    diagnostics += 1
                    return image, result.stdout
                report = decode(result.stdout)
                if mode == 'off' or scalar == 'off' or check:
                    assert all(value == 0 for value in report['inline'].values()), (name, mode, scalar, check, report)
                if not check:
                    builds += 1
                return image, report

            def execute(image, target, output, status):
                nonlocal executions, cross_builds
                if target == host:
                    result = call([image], status)
                    assert result.stdout == output, (image.name, target, result.stdout, output)
                    executions += 1
                else:
                    cross_builds += 1

            rows = {}
            for name, (source, output, status) in valid.items():
                for target in range(3):
                    pair = []
                    for mode in ('off', 'on'):
                        image, report = build(name, source, target, mode)
                        execute(image, target, output, status)
                        row = dict(case=name, target=TARGETS[target], mode=mode, bytes=image.stat().st_size,
                                   sha256=hashlib.sha256(image.read_bytes()).hexdigest(), **report)
                        rows[name, target, mode] = row
                        evidence['images'].append(row)
                        pair.append(report)
                    # Catalog/cache counters may differ. Bindings describe the
                    # original source and must remain exactly the same.
                    assert pair[0]['bindings'] == pair[1]['bindings'], (name, target, 'source bindings changed')
                    explicit_off = work / f'{name}-{target}-cli-off'
                    call([compiler, 'build', work / (name + '.ddr'), '--target', TARGETS[target],
                          '--inline-opt=off', '-o', explicit_off])
                    assert hashlib.sha256(explicit_off.read_bytes()).hexdigest() == rows[name, target, 'off']['sha256'], (name, target, 'explicit CLI-off output changed')
                    cli_checks += 1
                    if baseline:
                        older = work / f'{name}-{target}-baseline'
                        call([baseline, 'build', work / (name + '.ddr'), '--target', TARGETS[target], '-o', older])
                        assert older.read_bytes() == explicit_off.read_bytes(), (name, target, 'older baseline differs from explicit off')
                print('PASS effects/results/bindings', name, flush=True)
            for index, source in enumerate(invalid):
                for target in range(3):
                    for checking in (False, True):
                        _, before = build(f'invalid-{index}', source, target, 'off', check=checking, expected=1)
                        _, after = build(f'invalid-{index}', source, target, 'on', check=checking, expected=1)
                        assert before == after, (index, target, checking, 'diagnostic changed', before, after)
            # Scalar-off bypasses producer admission and keeps exact code.
            for target in range(3):
                source = valid['eager-repeated'][0]
                old, before = build('scalar-off', source, target, 'off', scalar='off')
                current, after = build('scalar-off', source, target, 'on', scalar='off')
                assert old.read_bytes() == current.read_bytes() and before['bindings'] == after['bindings']
                # Valid check mode must report bindings but do no inline work.
                _, before = build('checking', source, target, 'off', check=True)
                _, after = build('checking', source, target, 'on', check=True)
                assert before['bindings'] == after['bindings']
            # Native cold/warm/private-body identity. Reuse is expected; its
            # exact count and actual inline work depend on cache coverage.
            cache = work / 'cache'; cache.mkdir()
            source = 'fn private(x:i32)->i32{x+1} fn main(){print(private(8))}'
            first, cold = build('cache-A', source, host, 'on', cache=cache)
            first_bytes = first.read_bytes(); execute(first, host, '9\n', 0)
            second, warm = build('cache-A-warm', source, host, 'on', cache=cache)
            assert second.read_bytes() == first_bytes and warm['counters']['reused'] > 0
            changed = source.replace('x+1', 'x+2')
            third, changed_cold = build('cache-B', changed, host, 'on', cache=cache)
            execute(third, host, '10\n', 0)
            assert third.read_bytes() != first_bytes and changed_cold['counters']['reused'] == 0
            fourth, changed_warm = build('cache-B-warm', changed, host, 'on', cache=cache)
            assert fourth.read_bytes() == third.read_bytes() and changed_warm['counters']['reused'] > 0
            evidence['cache'] = dict(cold=cold, warm=warm, private_edit=changed_cold, private_edit_warm=changed_warm)
            assert rows['eager-unused', host, 'on']['inline']['calls'] == 1
            assert 0 < rows['caller-budget', host, 'on']['inline']['calls'] < 160
            assert 0 < rows['global-budget', host, 'on']['inline']['calls'] < 2000
            assert any(rows[name, target, 'on']['sha256'] != rows[name, target, 'off']['sha256']
                       for name in valid for target in range(3)), 'no actual code changed'
        assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest for path, digest in hashes.items()), 'compiler/source changed during suite'
        evidence.update(complete=True, valid_cases=len(valid), valid_builds=len(evidence['images']),
                        api_builds=builds, diagnostics=diagnostics, executions=executions,
                        cross_built_images=cross_builds, explicit_cli_off_checks=cli_checks)
        print(f'{len(evidence["images"])} frozen all-target images; {diagnostics} exact invalid diagnostics; '
              f'{executions} host executions; {cross_builds} other-target images; '
              f'{cli_checks} explicit CLI-off byte comparisons; scalar-off/check/cache contracts passed')
    except Exception as error:
        evidence['error'] = repr(error)
        raise
    finally:
        if args.report:
            args.report.write_text(json.dumps(evidence, indent=2, sort_keys=True) + '\n')


if __name__ == '__main__':
    main()
