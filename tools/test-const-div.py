#!/usr/bin/env python3
"""Verify constant division through consumer programs and independent integers.

Build every program for all four targets; execute the native host and Wasm.
--baseline optionally adds an older authoritative compiler, without new flags.
This is a correctness suite, not a timing or C-performance acceptance suite.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
ENV = {'PATH': '/no-external-tools'}
WASM = '''const fs=require('node:fs');
const print=x=>console.log(x.toString());
const program=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(process.argv[1])),
  {dunder:{print_i32:print,print_i64:print,print_bool:x=>print(Boolean(x))}}).exports;
try { const status=program.main(); if(status!==undefined)process.exitCode=Number(status); }
catch(error) {
  if(!(error instanceof WebAssembly.RuntimeError)||error.message!=='unreachable')throw error;
  process.exitCode=125;
}'''


def quotient(n, d):
    # Python // rounds down. Dunder rounds toward zero; never convert to float.
    return (abs(n) // abs(d)) * (-1 if (n < 0) != (d < 0) else 1)


def bounds(width):
    return -(1 << (width - 1)), (1 << (width - 1)) - 1


def divisors(width):
    low, high = bounds(width)
    result = {low, high, -high, -1, 1, -3, 3, -5, 5, -7, 7, -10, 10,
              -31, 31, -1009, 1009}
    # Both signs of every representable power of two, plus values on either
    # side of several powers. No benchmark constant is privileged.
    for bit in range(1, width - 1):
        result.update((1 << bit, -(1 << bit)))
    for bit in (2, 3, 5, 8, 15, width // 2, width - 2):
        for delta in (-1, 1):
            value = (1 << bit) + delta
            result.update((value, -value))
    rng = random.Random(0xD1A1DE + width)
    for _ in range(16):
        # Frozen dyadic-scale random odd and even divisors.
        bit = rng.randrange(2, width)
        value = rng.randrange(1 << (bit - 1), 1 << bit)
        result.add(value if value <= high else high)
        result.add(-value if -value >= low else low)
    return sorted(result)


def numerators(width, d):
    low, high = bounds(width)
    result = {low, low + 1, low + 2, high, high - 1, high - 2,
              -17, -7, -3, -2, -1, 0, 1, 2, 3, 7, 17}
    for q in (-17, -3, -2, -1, 0, 1, 2, 3, 17,
              quotient(low, d), quotient(high, d)):
        result.update(q * d + delta for delta in (-1, 0, 1))
    rng = random.Random(0xC0DE + width * 17 + d)
    for _ in range(24):
        bit = rng.randrange(width)
        value = rng.randrange(1 << bit, 1 << (bit + 1))
        result.update((value, -value))
    # MIN/-1 and MIN%-1 are tested as separate trapping programs.
    return sorted(n for n in result if low <= n <= high and not (n == low and d == -1))


def arithmetic_programs():
    for width in (32, 64):
        ty = f'i{width}'
        ds = divisors(width)
        # Keep individual compiler invocations and legacy main bodies bounded.
        for start in range(0, len(ds), 12):
            functions, body, expected = [], [], []
            for index, d in enumerate(ds[start:start + 12]):
                functions += [f'fn q{index}(n:{ty})->{ty}{{n/({d})}}',
                              f'fn r{index}(n:{ty})->{ty}{{n%({d})}}']
                ns = numerators(width, d)
                # Array iteration produces runtime local operands at every call;
                # each helper itself sees an unknown parameter, not a constant.
                body.append('{values:[]' + ty + '=[' + ','.join(map(str, ns)) + '];'
                            + f'for n in values{{print(q{index}(n));print(r{index}(n))}}' + '}')
                for n in ns:
                    q = quotient(n, d)
                    expected += [q, n - q * d]
            yield (f'i{width}_{start // 12:02}', '\n'.join(functions)
                   + '\nfn main(){' + ''.join(body) + '}\n', expected, len(functions))


def compound_programs():
    for width in (32, 64):
        ty = f'i{width}'
        source = f'''fn complex(a:{ty},b:{ty},c:{ty})->{ty}{{((a+b)*(c-1))/7}}
fn folded(a:{ty})->{ty}{{a/(3+4)}}
fn compound(a:{ty},b:{ty})->{ty}{{(a/3+b%7)/(-5)}}
fn nested(a:{ty})->{ty}{{(a/(-7))%3}}
fn shorted(a:{ty})->bool{{a==0 || a/0==1}}
fn choose(a:{ty})->{ty}{{if a<0{{return a/(-3)}}else{{return a%5}}}}
fn pressure(n:{ty})->{ty}{{'''
        source += ''.join(f'var a{i}:=n+{i};' for i in range(18))
        source += 'var i:=0;while i<4{'
        ds = [3, -5, 7, -11, 13, -17] * 3
        source += ''.join(f'a{i}=a{i}/{d};' for i, d in enumerate(ds))
        source += 'i=i+1};' + '+'.join(f'a{i}*{i+1}' for i in range(18)) + '}\n'
        source += f'fn eight(a:{ty},b:{ty},c:{ty},d:{ty},e:{ty},f:{ty},g:{ty},h:{ty})->{ty}'
        source += '{a/3+b/(-5)+c/7+d/(-11)+e/13+f/(-17)+g/19+h/(-23)}\nfn main(){'
        expected = []
        for a, b, c in [(-77777, 31, -7), (-101, -33, 9), (0, 7, 1), (17, -9, 3), (123456, 3, -2)]:
            source += '{var a:' + ty + f'={a};var b:{ty}={b};var c:{ty}={c};'
            source += 'print(complex(a,b,c));print(folded(a));print(compound(a,b));print(nested(a));print(choose(a));print(pressure(a))}'
            values = [a + i for i in range(18)]
            for _ in range(4):
                values = [quotient(n, d) for n, d in zip(values, ds)]
            expected += [quotient((a+b)*(c-1), 7), quotient(a, 7),
                         quotient(quotient(a, 3) + b - quotient(b, 7)*7, -5),
                         quotient(a, -7) - quotient(quotient(a, -7), 3)*3,
                         quotient(a, -3) if a < 0 else a - quotient(a, 5)*5,
                         sum(n*(i+1) for i, n in enumerate(values))]
        source += 'var zero:' + ty + '=0;print(shorted(zero));'
        expected.append(True)
        values = [91, -72, 53, -34, 25, -16, 7, -8]
        source += ''.join(f'var p{i}:{ty}={n};' for i, n in enumerate(values))
        source += 'print(eight(' + ','.join(f'p{i}' for i in range(8)) + '))}\n'
        expected.append(sum(quotient(n, d) for n, d in zip(values, [3,-5,7,-11,13,-17,19,-23])))
        yield f'compound_i{width}', source, expected, 8


def trap_programs():
    for width in (32, 64):
        ty = f'i{width}'; low, high = bounds(width)
        for operator, label in (('/', 'div'), ('%', 'rem')):
            for suffix, expression, argument, status in (
                    ('zero', f'n{operator}0', 7, 102),
                    ('minimum_zero', f'n{operator}0', low, 102),
                    ('minimum_negative_one', f'n{operator}(-1)', low, 101),
                    ('numerator_overflow_before_zero', f'(n+1){operator}0', high, 101),
                    ('numerator_overflow_before_one', f'(n+1){operator}1', high, 101),
                    ('numerator_underflow_before_one', f'(n-1){operator}1', low, 101),
                    ('nested_zero_before_one', f'(n/0){operator}1', 1, 102),
                    ('denominator_overflow', f'n{operator}({high}+1)', 0, 101)):
                source = f'fn f(n:{ty})->{ty}{{{expression}}}\nfn main(){{var n:{ty}={argument};print(17);value:=f(n);print(99)}}\n'
                yield f'{ty}_{label}_{suffix}', source, status
    for d in (0, 1, -1, 3):
        for op, label in (('/', 'div'), ('%', 'rem')):
            source = f'fn f(a:&[]i64,i:i32)->i64{{a[i]{op}({d})}}\nfn main(){{a:[]i64=[5];var i:=-1;print(17);value:=f(&a,i);print(99)}}\n'
            yield f'array_{label}_{d}', source, 103
    yield 'short_circuit_evaluated', 'fn f(n:i64)->bool{n==0||n/0==1}\nfn main(){var n:i64=1;print(17);value:=f(n);print(99)}\n', 102


def invalid_programs():
    for name, body in (
            ('type', 'fn f(n:i64)->i64{n/true}'),
            ('return', 'fn f(n:i64)->bool{n/3}'),
            ('immutable', 'fn f(n:i64)->i64{n=n/3;n}'),
            ('dead_type', 'fn f(n:i64)->i64{if false{return n/true};n/3}'),
            ('dead_literal_width', 'fn f(n:i32)->i32{if false{return n/2147483648};n/3}'),
            ('syntax', 'fn f(n:i64)->i64{n/}')):
        yield name, body + '\nfn main(){print(42)}\n'


def plan_program(module, directory):
    # Test the real self-hosted generator, never a second Python magic-number
    # generator. The Python evaluator below is only the documented bitvector
    # meaning of its returned plan; the oracle remains exact signed division.
    relative = os.path.relpath(module.with_suffix(''), directory).replace(os.sep, '/')
    source = 'import ' + json.dumps(relative) + ' as cd\n'
    source += '''fn dump(p:&mut cd.Plan,d:i64,width:i32){
        assert(cd.prepare_into(&mut p,d,width))
        assert(p.supported)
        var fresh:=cd.new();assert(cd.prepare_into(&mut fresh,d,width))
        assert(p.supported==fresh.supported && p.kind==fresh.kind && p.width==fresh.width &&
            p.divisor==fresh.divisor && p.magic==fresh.magic && p.shift==fresh.shift &&
            p.add==fresh.add && p.negative==fresh.negative && p.iterations==fresh.iterations)
        print(p.width);print(p.divisor);print(p.kind);print(p.magic)
        print(p.shift);print(p.add);print(p.negative);print(p.iterations)
    }
    fn main(){var p:=cd.new();'''
    pairs = []
    for width in range(3, 11):
        low, high = bounds(width)
        source += f'{{var d:i64={low};while d<={high}{{dump(&mut p,d,{width});d=d+1}}}}'
        pairs += [(width, d) for d in range(low, high + 1)]
    for width in (32, 64):
        ds = sorted(set(divisors(width) + [0]))
        source += '{values:[]i64=[' + ','.join(map(str, ds)) + '];'
        source += f'for d in values{{dump(&mut p,d,{width})}}' + '}'
        pairs += [(width, d) for d in ds]
    # An unsupported request must not leave the preceding supported plan active.
    for d, width in ((1, 2), (1, 65), (4, 3), (-5, 3)):
        source += f'assert(!cd.prepare_into(&mut p,{d},{width}));assert(!p.supported);'
        source += 'assert(p.kind==0 && p.magic==0 && p.shift==0 && !p.add && p.iterations==0);'
        source += 'dump(&mut p,3,32);'
        pairs.append((32, 3))
    return source + '}\n', pairs


def signed(value, width):
    value &= (1 << width) - 1
    return value - (1 << width) if value & (1 << (width - 1)) else value


def plan_quotient(n, d, width, kind, magic, shift, add, negative):
    low, _ = bounds(width)
    if d == 0:
        return 102
    if n == low and d == -1:
        return 101
    if kind == 2:
        return -n if negative else n
    if kind == 3:
        q = (n + ((1 << shift) - 1 if n < 0 else 0)) >> shift
    elif kind == 4:
        return int(n == low)
    else:
        assert kind == 5, (width, d, kind)
        q = (n * magic) >> width
        if add:
            q = signed(q + n, width)
        q = (q >> shift) - (-1 if n < 0 else 0)
    return -q if negative else q


def verify_plans(output, pairs):
    lines = output.decode('ascii').splitlines()
    assert len(lines) == len(pairs) * 8, ('plan export field count', len(lines), len(pairs) * 8)
    checked = 0
    for index, (width, d) in enumerate(pairs):
        fields = lines[index * 8:index * 8 + 8]
        assert fields[5] in ('true', 'false') and fields[6] in ('true', 'false'), fields
        w, divisor, kind, magic, shift = map(int, fields[:5])
        add, negative = fields[5] == 'true', fields[6] == 'true'
        iterations = int(fields[7])
        assert (w, divisor) == (width, d), fields
        low, high = bounds(width)
        assert low <= magic <= high and 0 <= shift < width and 0 <= iterations <= 128, fields
        expected_kind = (1 if d == 0 else 2 if abs(d) == 1 else 4 if d == low
                         else 3 if abs(d) & (abs(d) - 1) == 0 else 5)
        assert kind == expected_kind and negative == (d < 0), fields
        if kind == 3:
            assert abs(d) == 1 << shift, fields
        if kind == 5:
            multiplier = magic + ((1 << width) if add else 0)
            # Independently check the generator's mathematical certificate:
            # Theorem 5.1, ell=shift+1. Exact Python integers avoid the
            # generator's machine-width quotient/remainder recurrence entirely.
            error = multiplier * abs(d) - (1 << (width + shift))
            assert add == (magic < 0) and 0 < multiplier < 1 << width, fields
            assert 0 < error <= 1 << (shift + 1), (fields, 'invalid multiplier certificate', error)
        ns = range(low, high + 1) if width <= 10 else (
            sorted(set(numerators(width, d) + [low])) if d else [low, -1, 0, 1, high])
        for n in ns:
            actual = plan_quotient(n, d, width, kind, magic, shift, add, negative)
            if d == 0 or (n == low and d == -1):
                assert actual == (102 if d == 0 else 101), (n, fields, actual)
            else:
                expected = quotient(n, d)
                assert actual == expected, (n, fields, actual, expected)
                remainder = n - actual * d
                assert low <= actual <= high and abs(remainder) < abs(d), (n, fields, actual, remainder)
                assert remainder == 0 or (remainder < 0) == (n < 0), (n, fields, remainder)
            checked += 1
    return checked


def call(command, work):
    return subprocess.run([str(x) for x in command], cwd=work, env=ENV,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)


def image_header(path, target):
    data = path.read_bytes()
    if target == 'wasm32':
        assert data[:8] == b'\0asm\1\0\0\0', (target, 'invalid Wasm image')
    elif target.endswith('linux'):
        assert data[:6] == b'\x7fELF\x02\x01', (target, 'invalid ELF image')
        assert struct.unpack_from('<H', data, 18)[0] == (183 if target == 'arm64-linux' else 62), target
    else:
        assert data[:4] == b'\xcf\xfa\xed\xfe', (target, 'invalid Mach-O image')
        assert struct.unpack_from('<I', data, 4)[0] == 0x100000C, target


def output_bytes(values):
    return ''.join((str(v).lower() if type(v) is bool else str(v)) + '\n' for v in values).encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path, help='optional older compiler, receives no const-div flags')
    parser.add_argument('--node', default='node')
    parser.add_argument('--verify-selection', action='store_true', help='assert helpers still use the typed model via timings counters')
    parser.add_argument('--case', help='run only program names containing this substring')
    parser.add_argument('--plan-module', type=Path, default=ROOT / 'selfhost/const_div.ddr',
                        help='real Dunder plan module to export; default: this checkout selfhost/const_div.ddr')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    variants = [('off', compiler, ['--const-div-opt=off']),
                ('on', compiler, ['--const-div-opt=on'])]
    if args.baseline:
        variants.insert(0, ('baseline', args.baseline.resolve(strict=True), []))
    node = shutil.which(args.node)
    if not node:
        parser.error('Node is required for Wasm execution')
    node = Path(node).resolve(strict=True)
    if platform.system() == 'Darwin' and platform.machine() == 'arm64':
        native = 'arm64-macos'
    elif platform.system() == 'Linux' and platform.machine() in ('arm64', 'aarch64', 'x86_64'):
        native = 'x86_64-linux' if platform.machine() == 'x86_64' else 'arm64-linux'
    else:
        parser.error('execution requires an ARM64 macOS or ARM64/x86-64 Linux host')
    hashes = {p: hashlib.sha256(p.read_bytes()).hexdigest() for _, p, _ in variants}
    executed = crossed = diagnostics = result_values = programs = 0
    positives = list(arithmetic_programs()) + list(compound_programs())
    traps = list(trap_programs())
    invalids = list(invalid_programs())
    if args.case:
        positives = [p for p in positives if args.case in p[0]]
        traps = [p for p in traps if args.case in p[0]]
        invalids = [p for p in invalids if args.case in p[0]]
        if not positives and not traps and not invalids and args.case not in 'plans':
            parser.error('--case did not match any program')
    with tempfile.TemporaryDirectory(prefix='dunder-const-div-') as tmp:
        work = Path(tmp)
        plan_hash = None
        if not args.case or args.case in 'plans':
            module = args.plan_module.resolve(strict=True)
            plan_hash = hashlib.sha256(module.read_bytes()).hexdigest()
            source, pairs = plan_program(module, work)
            path = work / 'plans.ddr'; path.write_text(source)
            outputs = []
            # Compile the plan generator without its own constant-div lowering;
            # then verify every exported plan independently. Actual on/off
            # machine code is exercised by the consumer programs below.
            for target in TARGETS:
                image = work / ('plans-' + target)
                built = call([compiler, 'build', path, '--target', target, '-o', image, '--const-div-opt=off'], work)
                assert built.returncode == 0, ('plan export build', target, built.stderr.decode(errors='replace'))
                image_header(image, target)
                if target in (native, 'wasm32'):
                    result = call([image] if target == native else [node, '-e', WASM, image], work)
                    assert result.returncode == 0, ('plan export run', target, result.returncode, result.stderr)
                    outputs.append(result.stdout)
                    executed += 1
                else:
                    crossed += 1
            assert outputs[0] == outputs[1], 'host/Wasm exported different plans'
            checked = verify_plans(outputs[0], pairs)
            print(f'PASS {len(pairs)} real Dunder plans; {checked} exact plan inputs; exhaustive signed widths 3..10', flush=True)
            programs += 1
        for name, source, expected, helpers in positives + [(n,s,None,1) for n,s,_ in traps]:
            status = next((status for n, _, status in traps if n == name), 0)
            path = work / (name + '.ddr'); path.write_text(source)
            wanted = b'17\n' if expected is None else output_bytes(expected)
            for variant, binary, flags in variants:
                for target in TARGETS:
                    image = work / f'{name}-{variant}-{target}'
                    selection = ['--timings=json'] if args.verify_selection and variant != 'baseline' else []
                    built = call([binary, 'build', path, '--target', target, '-o', image, *flags, *selection], work)
                    assert built.returncode == 0, (name, variant, target, built.returncode, built.stderr.decode(errors='replace'))
                    image_header(image, target)
                    if selection:
                        data = json.loads(built.stderr)['optimizer']
                        assert data['enabled'] and data['emitted'] >= helpers, (name, variant, target, helpers, data)
                    if target in (native, 'wasm32'):
                        command = [image] if target == native else [node, '-e', WASM, image]
                        result = call(command, work)
                        wanted_status = 125 if target == 'wasm32' and status else status
                        assert result.returncode == wanted_status and result.stdout == wanted, (
                            name, variant, target, result.returncode, wanted_status, result.stdout[:1000], wanted[:1000], result.stderr)
                        executed += 1
                        if expected is not None:
                            result_values += len(expected)
                    else:
                        crossed += 1
            programs += 1
            print('PASS', name, 'trap' if expected is None else f'{len(expected)} independent results', flush=True)
        for name, source in invalids:
            path = work / ('invalid_' + name + '.ddr'); path.write_text(source)
            for target in TARGETS:
                for mode in ('check', 'check-agent', 'build'):
                    outcomes = []
                    for variant, binary, flags in variants:
                        image = work / 'previous'; image.write_bytes(b'previous output')
                        extra = ['--agent'] if mode == 'check-agent' else ['-o', image] if mode == 'build' else []
                        result = call([binary, 'check' if mode == 'check-agent' else mode, path,
                                       '--target', target, *extra, *flags], work)
                        assert result.returncode == 1, (name, variant, target, mode, result.returncode, result.stdout, result.stderr)
                        assert image.read_bytes() == b'previous output', (name, 'invalid build replaced output')
                        outcomes.append((result.returncode, result.stdout, result.stderr))
                        diagnostics += 1
                    assert all(x == outcomes[0] for x in outcomes), (name, target, mode, outcomes)
            programs += 1
            print('PASS diagnostics', name, flush=True)
        if plan_hash:
            assert hashlib.sha256(module.read_bytes()).hexdigest() == plan_hash, 'plan module changed during suite'
    assert all(hashlib.sha256(p.read_bytes()).hexdigest() == value for p, value in hashes.items()), 'compiler changed during suite'
    print(f'{programs} programs; {executed} native/Wasm executions; {crossed} other-target cross-builds; '
          f'{result_values} checked result lines; {diagnostics} matching invalid diagnostics')
    print('Native trap codes are exact; Wasm exposes unreachable only (125), with exact pre-trap stdout.')


if __name__ == '__main__':
    main()
