#!/usr/bin/env python3
"""Independent semantic MIR oracle and whole-function off/on consumer contracts.

No timing series. All four targets are built; only the native host and Wasm
execute. The scalar oracle reads semantic MIR columns exported by the actual
lift/verifier, never retained instruction words or the ARM encoding recipe.
Opaque operations, foreign calls and exhausted fuel are coverage gaps, not
successful interpreter runs. Whole-source tests separately exercise ownership,
calls, exact native traps, evaluation order and original source diagnostics.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
ENV = {'PATH': '/no-external-tools'}
MASK64 = (1 << 64) - 1
COLUMNS = ('kind', 'opcode', 'dst', 'a', 'b', 'c', 'imm', 'aux', 'type',
           'place', 'extension', 'effects', 'flags', 'abi', 'reads', 'writes',
           'clobbers', 'mode', 'target')
WASM = """const fs=require('node:fs');
const print=x=>console.log(x.toString());
const program=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(process.argv[1])),
 {dunder:{print_i32:print,print_i64:print,print_bool:x=>print(Boolean(x))}}).exports;
try {const result=program.main();if(process.argv[2]!=='ignore-result'&&result!==undefined)process.exitCode=Number(result)}
catch(error){if(!(error instanceof WebAssembly.RuntimeError)||error.message!=='unreachable')throw error;process.exitCode=125}
"""


class CoverageGap(Exception):
    """Semantics outside this deliberately closed interpreter subset."""


class Incomplete(Exception):
    """Fuel exhaustion is never a completed semantic test."""


def signed(value, width=64):
    value &= (1 << width) - 1
    return value - (1 << width) if value >> (width - 1) else value


def quotient(a, b):
    assert b != 0
    return (-1 if (a < 0) != (b < 0) else 1) * (abs(a) // abs(b))


class Interpreter:
    """Mutable physical scalar variables, byte memory, NZCV and explicit CFG.

    W writes zero-extend; X writes preserve all 64 bits. Arithmetic is machine
    arithmetic here. Checked *source* traps are explicit preceding instructions
    and are covered separately by whole-source executions.
    """
    def __init__(self, model, fuel=10000):
        self.model = model
        self.fuel = fuel
        self.steps = 0
        self.variables = {29: 0, 30: 0, 31: 1 << 20}
        self.memory = {}
        self.nzcv = None
        self.coverage = Counter()

    def read(self, variable):
        if variable == -1:
            return 0
        if variable not in self.variables:
            raise CoverageGap(f'read of uninitialized variable {variable}')
        return self.variables[variable]

    def write(self, variable, value, width=64):
        if variable != -1:
            self.variables[variable] = value & ((1 << width) - 1)

    def load(self, address, size):
        if any(address + i not in self.memory for i in range(size)):
            raise CoverageGap(f'uninitialized memory [{address}, {address + size})')
        return sum(self.memory[address + i] << (8 * i) for i in range(size))

    def store(self, address, value, size):
        for i in range(size):
            self.memory[address + i] = (value >> (8 * i)) & 255

    def condition(self, condition):
        if self.nzcv is None:
            raise CoverageGap('NZCV read before definition')
        n, z, c, v = self.nzcv
        choices = (z, not z, c, not c, n, not n, v, not v,
                   c and not z, not c or z, n == v, n != v,
                   not z and n == v, z or n != v, True, True)
        if not 0 <= condition < len(choices):
            raise CoverageGap(f'unknown NZCV condition {condition}')
        return choices[condition]

    def binary(self, op, width):
        mask = (1 << width) - 1
        a = self.read(op['a']) & mask
        b = (self.read(op['b']) if op['b'] >= 0 else op['imm']) & mask
        shift = op['aux'] & 63
        shift_kind = (op['aux'] >> 6) & 3
        # Shifted operands are semantic aux fields, never decoded encodings.
        if op['opcode'] in (1, 2, 8, 9, 10, 11, 12, 13):
            if shift_kind == 0:
                b = (b << shift) & mask
            elif shift_kind == 1:
                b >>= shift
            elif shift_kind == 2:
                b = (signed(b, width) >> shift) & mask
            elif shift:
                b = ((b >> shift) | (b << (width - shift))) & mask
        code = op['opcode']
        if code == 1:
            result = a + b
        elif code == 2:
            result = a - b
        elif code == 3:
            result = a * b
        elif code == 4:
            result = (signed(a, width) * signed(b, width)) >> width
        elif code == 5:
            result = (a * b) >> width
        elif code == 6:
            result = quotient(signed(a, width), signed(b, width)) if b else 0
        elif code == 7:
            result = a // b if b else 0
        elif code in (8, 11):
            result = a & (b if code == 8 else ~b)
        elif code in (9, 12):
            result = a | (b if code == 9 else ~b)
        elif code in (10, 13):
            result = a ^ (b if code == 10 else ~b)
        elif code == 14:
            result = a << (b % width)
        elif code == 15:
            result = a >> (b % width)
        elif code == 16:
            result = signed(a, width) >> (b % width)
        elif code == 17:
            result = self.read(op['c']) + a * b
        elif code == 18:
            result = self.read(op['c']) - a * b
        else:
            raise CoverageGap(f'binary opcode {code}')
        if op['flags'] & 2:
            bits = result & mask
            carry = overflow = False
            if code == 1:
                carry = result > mask
                math = signed(a, width) + signed(b, width)
                overflow = not -(1 << (width - 1)) <= math < 1 << (width - 1)
            elif code == 2:
                carry = a >= b
                math = signed(a, width) - signed(b, width)
                overflow = not -(1 << (width - 1)) <= math < 1 << (width - 1)
            elif code not in (8, 11):
                raise CoverageGap(f'flag-setting binary opcode {code}')
            self.nzcv = (bool(bits >> (width - 1)), bits == 0, carry, overflow)
        return result

    def run(self):
        model = self.model
        labels = dict(model['labels'])
        position = 0
        while position < len(model['order']):
            if self.steps >= self.fuel:
                raise Incomplete(f'fuel={self.fuel}, position={position}')
            self.steps += 1
            op = model['operations'][model['order'][position]]
            kind = op['kind']
            self.coverage[f'{kind}:{op["opcode"]}'] += 1
            next_position = position + 1
            width = 32 if op['type'] == 1 else 64
            if op['mode'] == 2:
                position = next_position
                continue
            if kind == 1:
                self.write(op['dst'], op['imm'], width)
            elif kind == 2:
                self.write(op['dst'], self.read(op['a']), width)
            elif kind == 3:
                value = self.read(op['a'])
                if op['opcode'] == 3:
                    result = (value & ~(65535 << op['aux'])) | op['imm']
                elif op['opcode'] in (1, 2):
                    rotate, top = op['aux'] & 63, op['aux'] >> 6
                    if not (0 <= rotate < width and 0 <= top < width):
                        raise CoverageGap('invalid semantic bitfield bounds')
                    if rotate <= top:
                        bits = top - rotate + 1
                        result = (value >> rotate) & ((1 << bits) - 1)
                    else:
                        bits = top + 1 + width - rotate
                        result = (value & ((1 << (top + 1)) - 1)) << (width - rotate)
                    if op['opcode'] == 1:
                        result = signed(result, bits)
                else:
                    raise CoverageGap(f'unary opcode {op["opcode"]}')
                self.write(op['dst'], result, width)
            elif kind in (4, 5, 16):
                result = self.binary(op, width)
                self.write(op['dst'], result, width)
            elif kind == 6:
                if op['a'] != -1 or op['b'] != -1:
                    raise CoverageGap('general Select operands outside closed CSET subset')
                self.write(op['dst'], int(self.condition(op['imm'])), width)
            elif kind in (7, 8):
                place = model['places'][op['place']]
                assert (place['offset'], place['width']) == (op['imm'], op['aux']), op
                base = op['a'] if kind == 7 else op['b']
                address = self.read(base) + op['imm']
                if kind == 7:
                    value = self.load(address, op['aux'])
                    if op['extension'] == 2:
                        value = signed(value, op['aux'] * 8)
                    self.write(op['dst'], value, width)
                else:
                    self.store(address, self.read(op['a']), op['aux'])
            elif kind == 10:
                stack = self.read(31) - 16
                self.store(stack, self.read(op['a']), 8)
                self.store(stack + 8, 0, 8)
                self.write(31, stack)
            elif kind == 11:
                stack = self.read(31)
                self.write(op['dst'], self.load(stack, 8))
                self.write(31, stack + 16)
            elif kind in (12, 13):
                taken = True
                if kind == 13:
                    if op['opcode'] == 1:
                        taken = self.condition(op['imm'])
                    elif op['opcode'] in (2, 3):
                        taken = (self.read(op['a']) == 0) == (op['opcode'] == 2)
                    else:
                        raise CoverageGap(f'conditional opcode {op["opcode"]}')
                if taken:
                    if op['target'] not in labels or labels[op['target']] < 0:
                        raise CoverageGap(f'external branch {op["target"]}')
                    next_position = labels[op['target']]
            elif kind == 15:
                return self.read(0)
            elif kind == 18:
                pass
            elif kind == 21 and op['abi'] == 1:
                # The documented FP/LR frame pair has explicit memory effects.
                if op['effects'] == 36:
                    stack = self.read(31) - 16
                    self.store(stack, self.read(29), 8)
                    self.store(stack + 8, self.read(30), 8)
                    self.write(31, stack)
                elif op['effects'] == 34:
                    stack = self.read(31)
                    self.write(29, self.load(stack, 8))
                    self.write(30, self.load(stack + 8, 8))
                    self.write(31, stack + 16)
                else:
                    raise CoverageGap(f'ABI frame effects {op["effects"]}')
            else:
                raise CoverageGap(f'kind={kind}, opcode={op["opcode"]}, abi={op["abi"]}')
            position = next_position
        raise CoverageGap('function fell off end without Return')


def decode_model(output):
    lines = iter(output.splitlines())
    assert next(lines) == 'MIR1', output
    count = int(next(lines)); assert 0 < count < 10000, count
    operations = [dict(zip(COLUMNS, (int(next(lines)) for _ in COLUMNS))) for _ in range(count)]
    assert next(lines) == 'PLACES'
    count = int(next(lines)); assert 0 <= count < 10000
    places = [dict(zip(('kind', 'offset', 'width'), (int(next(lines)) for _ in range(3)))) for _ in range(count)]
    assert next(lines) == 'LABELS'
    count = int(next(lines)); assert 0 <= count < 10000
    labels = [(int(next(lines)), int(next(lines))) for _ in range(count)]
    assert next(lines) == 'ORDER'
    count = int(next(lines)); assert count == len(operations)
    order = [int(next(lines)) for _ in range(count)]
    assert sorted(order) == list(range(count))
    assert next(lines) == 'END' and next(lines, None) is None
    return dict(operations=operations, places=places, labels=labels, order=order)


def scalar_cases():
    # Frozen data, not a magic-number oracle: Python unbounded mathematical
    # definitions, then explicit machine-width truncation where specified.
    cases = []
    random_source = random.Random(0x4D495231)
    names = ('add', 'sub', 'mul', 'signed-high', 'unsigned-high', 'div',
             'and', 'or', 'xor', 'lsl', 'lsr', 'asr', 'stack', 'condition', 'select')
    for name in names:
        inputs = [(-9223372036854775808, 3), (9223372036854775807, -7),
                  (random_source.randrange(-(1 << 63), 1 << 63), random_source.randrange(-63, 64) or 1)]
        if name in ('condition', 'select'):
            inputs += [(0, 0), (-1, 0), (0, -1)]
        for a, b in inputs:
            ua, ub = a & MASK64, b & MASK64
            if name == 'add': result = a + b
            elif name in ('sub', 'stack'): result = a - b
            elif name == 'mul': result = a * b
            elif name == 'signed-high': result = (a * b) >> 64
            elif name == 'unsigned-high': result = (ua * ub) >> 64
            elif name == 'div': result = quotient(a, b)
            elif name == 'and': result = ua & ub
            elif name == 'or': result = ua | ub
            elif name == 'xor': result = ua ^ ub
            elif name == 'lsl': result = ua << (ub % 64)
            elif name == 'lsr': result = ua >> (ub % 64)
            elif name == 'asr': result = a >> (ub % 64)
            elif name == 'condition': result = 29 if a < b else 17
            elif name == 'select': result = int(a < b)
            cases.append((name, a, b, signed(result)))
    for a in (0, -1, 0x80000000, 0xFEDCBA9876543210 - (1 << 64)):
        cases.extend((('copy-w', a, 0, a & 0xFFFFFFFF),
                      ('sign-w', a, 0, signed(a, 32)),
                      ('signed-load', a, 0, signed(a, 32))))
    for a, b in ((2147483647, 3), (-2147483648, -7), (-1, 33)):
        ua, ub = a & 0xFFFFFFFF, b & 0xFFFFFFFF
        for name, result in (('add-w', a + b), ('sub-w', a - b),
                             ('mul-w', a * b), ('div-w', quotient(a, b)),
                             ('lsl-w', ua << (ub % 32)), ('lsr-w', ua >> (ub % 32)),
                             ('asr-w', a >> (ub % 32))):
            cases.append((name, a, b, result & 0xFFFFFFFF))
    for a, b in ((0x1122334455667788, -1), (-1, 0), (0, 0x80ABCDEF)):
        content = bytearray((a & MASK64).to_bytes(8, 'little'))
        content[2:6] = (b & 0xFFFFFFFF).to_bytes(4, 'little')
        cases.append(('overlap', a, b, signed(int.from_bytes(content, 'little'))))
    for n in (0, 1, 2, 17):
        cases.append(('loop', n, 0, sum(range(1, n + 1))))
    return cases


def source_cases():
    valid = {
        'mixed-calls-loop': ('fn bump(x:i32)->i32{x+3} fn run(n:i32)->i32{var a:[]i32=[];var text: string="";var sum:=0;for i in 0..n{a.push(bump(i));text=text+"a";sum=sum+a[i]};print(text.len);sum} fn main(){print(run(6))}', '6\n33\n', 0),
        'owned-return': ('struct Box{values:[]i32,label:string} fn make(n:i32)->Box{var a:[]i32=[];for i in 0..n{a.push(i*2)};Box{values:a,label:"box"}} fn main(){var a:=make(4);b:=a;a.values[0]=19;a.label=a.label+"!";print(b.values[0]);print(a.values[0]);print(b.label.len);print(a.label.len)}', '0\n19\n3\n4\n', 0),
        'array-snapshot': ('fn change(a:&mut []i32){for i in 0..a.len{a[i]=a[i]+7}} fn main(){var a:[]i32=[1,2,3];b:=a;change(&mut a);print(a[0]);print(b[0]);print(a[2]);print(b[2])}', '8\n1\n10\n3\n', 0),
        'owned-break-continue': ('fn run(n:i32)->i32{var sum:=0;for i in 0..n{s:="ab"+"c";a:[]i32=[i,i+1];if i==2{continue};if i==5{break};sum=sum+a[0]+s.len};sum} fn main(){print(run(10))}', '20\n', 0),
        'owned-early-return': ('fn make(n:i32)->string{for i in 0..n{a:[]string=["a","b"];if i==2{return a[1]+"ok"}};"none"} fn main(){print(make(5).len);print(make(1).len)}', '3\n4\n', 0),
        'nested-owned-alias': ('struct Box{a:[]string} fn main(){var a:=Box{a:["one","two"]};b:=a;a.a[0]="longer";print(a.a[0].len);print(b.a[0].len);print(b.a[1].len)}', '6\n3\n3\n', 0),
        'float-owned-call': ('fn calc(x:f64)->f64{x*1.5+2.25} fn main(){label:="mixed";a:[]f64=[calc(2.0),calc(-4.0)];assert(a[0]==5.25);assert(a[1]==-3.75);print(label.len)}', '5\n', 0),
        'indirect-owned-call': ('fn suffix(s:string)->string{s+"x"} fn main(){call:=suffix;print(call("ab").len)}', '3\n', 0),
        'eight-args-calls': ('fn mix(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32)->i32{h+a+d+f+c+b+e+g} fn effect(x:i32)->i32{print(x);x} fn main(){print(mix(effect(1),effect(2),effect(3),effect(4),effect(5),effect(6),effect(7),effect(8)))}', '1\n2\n3\n4\n5\n6\n7\n8\n36\n', 0),
        'short-circuit-calls': ('fn effect(x:i32)->bool{print(x);x>0} fn main(){print(false&&effect(1));print(true||effect(2));print(effect(3)&&effect(4));print(effect(0)||effect(5))}', 'false\ntrue\n3\n4\ntrue\n0\n5\ntrue\n', 0),
        'for-bound-snapshot': ('fn main(){var n:=5;var sum:=0;for i in 0..n{n=1;sum=sum+i};print(sum);print(n)}', '10\n1\n', 0),
        'while-backedge': ('fn step(x:i32)->i32{x-1} fn main(){var i:=9;var sum:=0;while i>0{i=step(i);if i==4{continue};sum=sum+i};print(sum)}', '32\n', 0),
        'shadow-locals': ('fn effect(x:i32)->i32{x+1} fn main(){var x:=3;if x>0{x:=effect(6);print(x)};print(x)}', '7\n3\n', 0),
        'division-rem': ('fn calc(a:i64,b:i64)->i64{a/b*17+a%b} fn main(){print(calc(-9234567890123,37));print(calc(9234567890123,-37))}', None, 0),
        'arg-trap-order': ('fn first(a:i32,b:i32)->i32{a} fn mark()->i32{print(71);7} fn zero()->i32{0} fn main(){print(first(mark(),9/zero()))}', '71\n', 102),
        'owned-arg-trap-order': ('fn use(a:string,b:i32)->i32{a.len+b} fn mark()->string{print(72);"abc"} fn zero()->i32{0} fn main(){print(use(mark(),9/zero()))}', '72\n', 102),
        'array-trap-before-call': ('fn effect()->i32{print(99);1} fn main(){a:[]i32=[1];print(a[-1]+effect())}', '', 103),
        'overflow-before-zero': ('fn max()->i32{print(73);2147483647} fn zero()->i32{print(99);0} fn main(){print((max()+1)/zero())}', '73\n', 101),
        'child-overflow-parent-zero': ('fn max()->i32{print(74);2147483647} fn main(){print((max()*2147483647)*0)}', '74\n', 101),
        'min-div': ('fn value()->i64{print(75);-9223372036854775808} fn main(){print(value()/(-1))}', '75\n', 101),
        'min-rem': ('fn value()->i32{print(76);-2147483648} fn main(){print(value()%(-1))}', '76\n', 101),
        'loop-late-overflow': ('fn main(){var i:=2147483646;while i>0{print(i);i=i+1}}', '2147483646\n2147483647\n', 101),
        'assert-order': ('fn effect()->bool{print(77);false} fn main(){var a:[]string=["owned"];assert(effect());print(a.len)}', '77\n', 104),
    }
    source, _, status = valid['division-rem']
    output = ''.join(str(quotient(a, b) * 17 + a - quotient(a, b) * b) + '\n'
                     for a, b in ((-9234567890123, 37), (9234567890123, -37)))
    valid['division-rem'] = source, output, status
    # More live values than the available GP register set, with a call barrier.
    declarations = ''.join(f'v{i}:=effect({i});' for i in range(36))
    total = '+'.join(f'v{i}' for i in range(36))
    valid['spill-call'] = ('fn effect(x:i32)->i32{x+2} fn main(){' + declarations + f'print({total})' + '}', str(sum(i + 2 for i in range(36))) + '\n', 0)
    invalid = [
        'fn main(){if false{missing()}}',
        'fn main(){if false{var a:[]i32=[true]}}',
        'fn main(){x:=1;if false{x=2}}',
        'fn bad()->i32{if false{return true};3} fn main(){}',
        'fn bad(x:i32)->i32{if x>0{return 3}} fn main(){}',
        'fn main(){if false{print(true<false)}}',
        'fn main(){a:[]i32=[1];a[0]=2}',
        'fn bad(x:i32,x:i32)->i32{x} fn main(){}',
        'fn main(){if false{print(2147483648)}}',
        'fn main(){if false{var a:=;}}',
    ]
    return valid, invalid


def fixture_source(name, source_root, work):
    text = (ROOT / 'tests/fixtures' / name).read_text()
    def replace(match):
        module, alias = match.groups()
        relative = os.path.relpath(source_root / 'selfhost' / module, work).replace(os.sep, '/')
        return 'import ' + json.dumps(relative) + ' as ' + (alias or module)
    return re.sub(r'import \.\.\.selfhost\.(\w+)(?: as (\w+))?', replace, text)


def decode_consumer(output):
    lines = output.splitlines()
    assert lines[0] == 'MIR' and lines[4] == 'OPT' and lines[12] == 'BINDINGS', output
    counters = dict(zip(('functions', 'operations', 'opaque'), map(int, lines[1:4])))
    assert all(value >= 0 for value in counters.values()), counters
    assert lines[5] in ('true', 'false'), output
    optimization = dict(zip(('optimized', 'promoted', 'removed_loads', 'removed_stores',
                             'analysis_units', 'budget_refusals'), map(int, lines[6:12])))
    assert all(value >= 0 for value in optimization.values()), optimization
    optimization['enabled'] = lines[5] == 'true'
    count = int(lines[13]); assert count >= 0 and len(lines) == 14 + 3 * count, output
    bindings = [(int(lines[i]), lines[i + 1], lines[i + 2]) for i in range(14, len(lines), 3)]
    return dict(mir=counters, optimization=optimization, bindings=bindings)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--only', choices=('all', 'oracle', 'source'), default='all')
    parser.add_argument('--node', default='node')
    parser.add_argument('--fuel', type=int, default=10000)
    parser.add_argument('--report', type=Path, help='JSON hashes, raw commands, models, counts and coverage limits')
    args = parser.parse_args()
    if args.fuel < 100:
        parser.error('--fuel must be at least 100')
    compiler = args.compiler.resolve(strict=True)
    source_root = args.source_root.resolve(strict=True)
    node = shutil.which(args.node)
    if not node:
        parser.error('Node is required for Wasm execution')
    node = Path(node).resolve(strict=True)
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64': host = 0
    elif system == 'Linux' and machine in ('aarch64', 'arm64'): host = 1
    elif system == 'Linux' and machine == 'x86_64': host = 2
    else: parser.error('native execution requires ARM64 macOS or ARM64/x86-64 Linux')
    inputs = {compiler, Path(__file__).resolve(), *source_root.glob('selfhost/*.ddr'),
              ROOT / 'tests/fixtures/mir-consumer.ddr', ROOT / 'tests/fixtures/mir-oracle.ddr'}
    hashes = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    evidence = dict(complete=False, scope=args.only, hashes=hashes, host=platform.uname()._asdict(), commands=[],
                    oracle=[], sources=[], fuel=args.fuel, limitations=[
                        'Nonhost Linux/macOS images are cross-built, not executed',
                        'Wasm exposes unreachable125, so exact trap reasons are native evidence only',
                        'Interpreter covers scalar/frame/CFG, not arbitrary calls, ARC, FP or external memory',
                        'Interpreted functions are closed generated kernels; owned source functions have runtime differential coverage',
                        'Canonical lowering changes only Constant/Copy/Load/Store in this cohort; other operations retain their encodings',
                        'Whole-source executions cover owned/call semantics independently of the closed interpreter subset',
                        'Raw division excludes zero and MIN/-1; checked source cases cover their trap semantics',
                        'Deterministic finite inputs and fuel do not constitute a universal proof'])
    counts = Counter()
    try:
        with tempfile.TemporaryDirectory(prefix='dunder-mir-qa-') as directory:
            work = Path(directory).resolve()

            def call(command, expected=0):
                result = subprocess.run([str(x) for x in command], cwd=work, env=ENV,
                                        capture_output=True, text=True, timeout=180)
                evidence['commands'].append(dict(command=[str(x) for x in command],
                                                status=result.returncode, expected=expected,
                                                stdout=result.stdout, stderr=result.stderr))
                assert result.returncode == expected, (command, expected, result.returncode, result.stdout, result.stderr)
                return result

            def execute(image, target, output='', status=0, ignore_result=False):
                if target not in (host, 3):
                    counts['cross_builds'] += 1
                    return
                command = [image] if target == host else [node, '-e', WASM, image, 'ignore-result' if ignore_result else 'result']
                result = call(command, 125 if target == 3 and status else status)
                assert result.stdout == output, (image, result.stdout, output)
                counts['executions'] += 1

            def build_fixture(name):
                path = work / name
                path.write_text(fixture_source(name, source_root, work))
                runner = path.with_suffix('')
                call([compiler, 'build', path, '-o', runner])
                return runner

            if args.only in ('all', 'oracle'):
                runner = build_fixture('mir-oracle.ddr')
                for index, (name, a, b, expected) in enumerate(scalar_cases()):
                    for target in range(4):
                        images = {}
                        for mode in ('plain', 'roundtrip', 'canonical'):
                            image = work / f'oracle-{index}-{target}-{mode}'
                            result = call([runner, image, target, mode, name, a, b, expected])
                            assert not result.stderr, result.stderr
                            model = decode_model(result.stdout)
                            interpreter = Interpreter(model, args.fuel)
                            try:
                                actual = interpreter.run()
                            except Incomplete:
                                counts['unexpected_incomplete'] += 1
                                raise
                            assert actual == expected & MASK64, (name, a, b, actual, expected, model)
                            counts['completed_interpreter_runs'] += 1
                            counts['max_steps'] = max(counts['max_steps'], interpreter.steps)
                            execute(image, target, ignore_result=True)
                            images[mode] = hashlib.sha256(image.read_bytes()).hexdigest()
                            counts['oracle_images'] += 1
                            evidence['oracle'].append(dict(case=name, left=a, right=b, expected=expected,
                                target=TARGETS[target], mode=mode, sha256=images[mode], steps=interpreter.steps,
                                coverage=dict(interpreter.coverage), model=model))
                        assert images['plain'] == images['roundtrip'], (name, target, 'retained roundtrip changed bytes')
                # Canonical metadata must describe the new operation exactly.
                # In particular, a proved frame load has effects2, not the
                # retained raw load's conservative memory+trap effects3.
                for field in ('effects', 'type', 'extension', 'reads', 'writes', 'clobbers'):
                    for target in range(4):
                        image = work / f'reject-{field}-{target}'
                        result = call([runner, image, target, 'reject-' + field, 'signed-load', -1, 0, -1])
                        assert result.stdout == 'REJECTED\n' and not image.exists(), (field, target, result.stdout)
                        counts['canonical_metadata_rejections'] += 1
                # Explicit noncoverage and bounded execution are exercised and
                # counted separately, never added to completed interpreter runs.
                for name, exception in (('opaque', CoverageGap), ('external', CoverageGap), ('cycle', Incomplete)):
                    image = work / name
                    result = call([runner, image, host, 'plain', name, 1, 2, 3])
                    model = decode_model(result.stdout)
                    try:
                        Interpreter(model, 100).run()
                    except exception as error:
                        counts['expected_incomplete' if exception is Incomplete else 'expected_coverage_gaps'] += 1
                        evidence['oracle'].append(dict(case=name, expected_gap=str(error), model=model))
                    else:
                        raise AssertionError((name, 'unsupported/fuel case falsely completed'))
                    assert not image.exists()
                # Negative control: the runtime assertion really compares all
                # bits, rather than merely returning success unconditionally.
                for target in range(4):
                    image = work / f'wrong-result-{target}'
                    result = call([runner, image, target, 'roundtrip', 'add', 1, 2, 4])
                    try:
                        Interpreter(decode_model(result.stdout), args.fuel).run()
                    except CoverageGap as error:
                        assert 'external branch' in str(error), error
                    else:
                        raise AssertionError('bad expected result escaped independent comparison')
                    execute(image, target, status=104, ignore_result=True)
                    counts['negative_runtime_controls'] += 1
                print('PASS semantic MIR scalar/frame/CFG oracle; explicit gaps and fuel limit', flush=True)

            if args.only in ('all', 'source'):
                runner = build_fixture('mir-consumer.ddr')
                valid, invalid = source_cases()

                def build(name, source, target, mode, checking=False, invalid=False):
                    path = work / (name + '.ddr'); path.write_text(source)
                    image = work / f'{name}-{target}-{mode}'
                    sentinel = b'previous image'
                    if checking or invalid: image.write_bytes(sentinel)
                    result = call([runner, path, image, target, mode, 'check' if checking else 'build'], int(invalid))
                    assert not result.stderr, result.stderr
                    if checking or invalid: assert image.read_bytes() == sentinel
                    if invalid:
                        assert result.stdout.startswith('ERROR\n'), result.stdout
                        counts['diagnostics'] += 1
                        return image, result.stdout
                    report = decode_consumer(result.stdout)
                    if checking or mode == 'off': assert not any(report['mir'].values()), report
                    else:
                        functions = len(re.findall(r'\bfn\s+\w+', source))
                        assert report['mir']['functions'] == functions and report['mir']['operations'] > 0, (name, target, functions, report)
                    opt = report['optimization']
                    assert opt['enabled'] is (mode == 'opt' and not checking), report
                    if mode != 'opt' or checking: assert not any(opt.values()), report
                    counts['source_optimized_functions'] += opt['optimized']
                    return image, report

                for name, (source, output, status) in valid.items():
                    for target in range(4):
                        reports = []
                        images = []
                        for mode in ('off', 'on', 'opt'):
                            image, report = build(name, source, target, mode)
                            execute(image, target, output, status)
                            digest = hashlib.sha256(image.read_bytes()).hexdigest()
                            evidence['sources'].append(dict(case=name, source=source, expected_stdout=output,
                                native_status=status, target=TARGETS[target], mode=mode, sha256=digest, **report))
                            reports.append(report); images.append(digest)
                            counts['source_images'] += 1
                        assert reports[0]['bindings'] == reports[1]['bindings'] == reports[2]['bindings'], (name, target, 'bindings changed')
                        assert images[0] == images[1], (name, target, 'passthrough bytes changed')
                    print('PASS whole source off/on', name, flush=True)
                for index, source in enumerate(invalid):
                    for target in range(4):
                        for checking in (False, True):
                            _, before = build(f'invalid-{index}', source, target, 'off', checking, True)
                            _, after = build(f'invalid-{index}', source, target, 'on', checking, True)
                            _, optimized = build(f'invalid-{index}', source, target, 'opt', checking, True)
                            assert before == after == optimized, (index, target, checking, before, after, optimized)
                for target in range(4):
                    _, before = build('valid-check', valid['mixed-calls-loop'][0], target, 'off', True)
                    _, after = build('valid-check', valid['mixed-calls-loop'][0], target, 'on', True)
                    _, optimized = build('valid-check', valid['mixed-calls-loop'][0], target, 'opt', True)
                    assert before['bindings'] == after['bindings'] == optimized['bindings']
                assert counts['source_optimized_functions'] > 0, 'source pass was never activated'
                counts['valid_source_cases'] = len(valid)
                counts['invalid_source_cases'] = len(invalid)
                print('PASS exact source reports, invalid diagnostics and check-only image preservation', flush=True)

        assert all(hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest for path, digest in hashes.items()), 'compiler/source changed during suite'
        evidence.update(complete=True, counts=dict(counts))
        print(json.dumps(dict(counts), sort_keys=True))
    except Exception as error:
        evidence.update(error=repr(error), counts=dict(counts))
        raise
    finally:
        if args.report:
            args.report.write_text(json.dumps(evidence, indent=2, sort_keys=True) + '\n')


if __name__ == '__main__':
    main()
