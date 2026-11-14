#!/usr/bin/env python3
"""Build and execute runtime contracts with a Dunder compiler."""
import argparse
import json
import math
import os
from pathlib import Path
import random
import struct
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/contracts'
BOUNDED = {
    'array_ownership', 'array_returns', 'array_snapshots', 'block_cleanup', 'lexical_cleanup',
    'nested_arrays', 'nested_ownership', 'string_elements', 'string_returns', 'string_snapshots',
    'temporary_transfers',
}
# The program emits a marker before the invalid operation. A compiler error,
# early trap, later assertion or crash cannot satisfy the contract.
TRAPS = {
    'i32_overflow': (101, 'x := 2147483647; unused := x + 1'),
    'i64_overflow': (101, 'x: i64 = 9223372036854775807; unused := x * 2'),
    'min_rem': (101, 'x: i64 = -9223372036854775808; unused := x % -1'),
    'unsigned_overflow': (101, 'x: u64 = 18446744073709551615; unused := x + 1'),
    'unsigned_narrow': (101, 'x: u16 = 256; unused := u8(x)'),
    'zero_divisor': (102, 'x := 0; unused := 1 / x'),
    'empty_pop': (103, 'var values: []i32 = []; unused := values.pop()'),
    'bad_index': (103, 'values := [1, 2]; unused := values[-1]'),
    'string_bounds': (103, 'unused := "å"[0..3]'),
    'assertion': (104, 'assert(false)'),
    'string_capacity': (105, 'var text := "x"; for i in 0..25 { text = text + text }'),
    'utf8_boundary': (106, 'unused := "å"[1..2]'),
    'empty_slice_boundary': (106, 'unused := "å"[1..1]'),
    'negative_rune': (106, 'unused := rune(-1)'),
    'rune_surrogate': (106, 'unused := rune(55296)'),
    'rune_out_of_range': (106, 'unused := rune(1114112)'),
    'invalid_float_cast': (107, 'unused := i64(0.0 / 0.0)'),
    'invalid_enum_tag': (108, 'unused := Kind(2)'),
    'negative_capacity': (109, 'var values: []i32 = []; values.reserve(-1)'),
    'excess_capacity': (109, 'var values: []i64 = []; values.reserve(2147483647)'),
}


def numeric_reference():
    rng = random.Random(42)
    lines = []
    main = []
    for width in (32, 64):
        ty = f'i{width}'
        # Division truncates toward zero. Python's // on signed operands floors.
        lines.append(f'fn divide{width}(a:{ty},b:{ty},q:{ty},r:{ty}) '
                     '{ assert(a/b == q); assert(a%b == r) }')
        lo, hi = -(1 << (width - 1)), (1 << (width - 1)) - 1
        for divisor in (2, 3, 7, 1009, hi, -3):
            values = [lo, lo + 1, hi, -1, 0, 1, divisor]
            values += [rng.randint(lo, hi) for _ in range(16)]
            for value in values:
                q = abs(value) // abs(divisor)
                if (value < 0) != (divisor < 0):
                    q = -q
                main.append(f'divide{width}({value},{divisor},{q},{value - q * divisor})')

    # Decode and round with the host's independent IEEE binary32/binary64 codec.
    for width, fmt, signed in ((32, 'f', 'i'), (64, 'd', 'q')):
        ty = f'f{width}'
        lines.append(f'fn float{width}(a:{ty},b:{ty},s:i{width},p:i{width}) '
                     '{ assert((a+b).to_bits()==s); assert((a*b).to_bits()==p) }')

        def bits(value):
            return struct.unpack('>' + signed, struct.pack('>' + fmt, value))[0]

        def decode(value):
            return struct.unpack('>' + fmt, struct.pack('>' + signed, value))[0]

        # Include both signs of zero and subnormals. Restrict maximum exponent
        # so that both operations have finite results on the selected width.
        unit = 23 if width == 32 else 52
        bias = 127 if width == 32 else 1023
        seeds = [0, -(1 << (width - 1)), 1, 2, bits(0.1), bits(-2.75)]
        for _ in range(36):
            magnitude = (rng.randrange(1, bias + 15) << unit) | rng.getrandbits(unit)
            seeds.append(magnitude - (1 << (width - 1)) if rng.randrange(2) else magnitude)
        for i, a in enumerate(seeds):
            for b in (seeds[(i + 1) % len(seeds)], seeds[4]):
                x, y = decode(a), decode(b)
                main.append(f'float{width}({ty}.from_bits({a}),{ty}.from_bits({b}),'
                            f'{bits(x + y)},{bits(x * y)})')
    return '\n'.join([*lines, 'fn main() {', *main, 'print(42)', '}'])


def unicode_reference():
    # Python ord() and its UTF-8 codec check both Dunder iteration modes.
    text = 'A\0' + ''.join(chr(n) for n in (0x7F, 0x80, 0x7FF, 0x800, 0xD7FF,
                                             0xE000, 0xFFFF, 0x10000, 0x10FFFF))
    text += 'a\u030aå🌍'
    data = text.encode('utf-8')
    literal = json.dumps(text, ensure_ascii=False).replace('\\u0000', '\\0')
    return f'''fn main() {{
        text := {literal}
        bytes := {list(data)}
        scalars := {[ord(c) for c in text]}
        assert(text.len == bytes.len)
        var i := 0
        for b in text.bytes() {{ assert(b == bytes[i]); i = i + 1 }}
        assert(i == bytes.len)
        var j := 0
        var offset := 0
        for r in text.runes() {{
            assert(i32(r) == scalars[j]); assert(rune(scalars[j]) == r)
            end := offset + r.str().len
            assert(text[offset..end] == r.str())
            offset = end; j = j + 1
        }}
        assert(offset == bytes.len); assert(j == scalars.len)
        print(42)
    }}'''


def float_format_reference(directory):
    # Use byte output to exercise Dunder's float formatter directly.
    rng = random.Random(1234)
    lines = ['fn show(text: string) { for b in text.bytes(){ print(b) }; print(-1) }',
             'fn main(){']
    numbers = []
    for width, fmt, signed in ((32, 'f', 'i'), (64, 'd', 'q')):
        bits = [0, 1, -(1 << (width - 1)), (1 << (23 if width == 32 else 52))]
        bits += [rng.randrange(-(1 << (width - 1)), 1 << (width - 1)) for _ in range(192)]
        for raw in bits:
            value = struct.unpack('>' + fmt, struct.pack('>' + signed, raw))[0]
            if math.isfinite(value):
                lines.append(f'show(f{width}.from_bits({raw}).str())')
                numbers.append((fmt, signed, raw))
    path = directory / 'float_format.ddr'
    path.write_text('\n'.join([*lines, 'print(42)', '}']), encoding='utf-8')
    return path, numbers


def check_formatted(output, numbers):
    records = output.split(b'-1\n')
    if len(records) != len(numbers) + 1 or records[-1] != b'42\n':
        raise ValueError('float formatter emitted incomplete values')
    for record, (fmt, signed, expected) in zip(records, numbers):
        rendered = bytes(int(n) for n in record.splitlines())
        value = struct.unpack('>' + signed, struct.pack('>' + fmt, float(rendered)))[0]
        if value != expected:
            raise ValueError(f'{fmt} bits {expected} formatted as {rendered!r}, decoded as {value}')


def collections_reference():
    rng = random.Random(123)
    # Exercise empty keys, equivalent-looking Unicode, common prefixes, hash
    # collisions, replacements, tombstones, growth and iteration after removal.
    keys = ['', 'a', 'i', 'q', 'y', 'å', 'a\u030a', '🌍', 'a\0b']
    keys += [f'key-{n}' for n in range(200)]
    values, present = {}, set()
    lines = ['import std.map', 'import std.set', 'import std.array',
             'fn main(){', 'var m:=map.new(); var s:=set.new(); var all:[]string=[]']
    for step in range(900):
        key = rng.choice(keys)
        lit = json.dumps(key, ensure_ascii=False).replace('\\u0000', '\\0')
        op = rng.randrange(4)
        if op <= 1:
            value = rng.randrange(-5000, 5001)
            fresh = str(key not in present).lower()
            lines.append(f'map.put(&mut m,{lit},{value}); assert(set.insert(&mut s,{lit})=={fresh})')
            values[key] = value
            present.add(key)
        elif op == 2:
            removed = str(key in present).lower()
            lines.append(f'assert(map.remove(&mut m,{lit})=={removed}); assert(set.remove(&mut s,{lit})=={removed})')
            values.pop(key, None)
            present.discard(key)
        else:
            lines.append(f'assert(map.get_or(&m,{lit},-999999)=={values.get(key,-999999)}); '
                         f'assert(set.contains(&s,{lit})=={str(key in present).lower()})')
        if step % 97 == 0:
            lines.append(f'assert(map.length(&m)=={len(values)}); assert(set.length(&s)=={len(present)})')
    lines += ['var sum:i64=0', 'for key in map.keys(&m){all.push(key);sum=sum+map.get_or(&m,key,0)}',
              f'assert(sum=={sum(values.values())}); assert(all.len=={len(values)})',
              'array.sort_strings(&mut all)']
    for i, key in enumerate(sorted(values, key=lambda k: k.encode())):
        lit = json.dumps(key, ensure_ascii=False).replace('\\u0000', '\\0')
        lines.append(f'assert(all[{i}]=={lit})')
    return '\n'.join([*lines, 'print(42)', '}'])


def run_binary(command, directory, timeout):
    # wait4 returns this child's high-water RSS, even when it exits between
    # samples. Compiler memory usage cannot contaminate the program's bound.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        with subprocess.Popen(command, cwd=directory, stdin=subprocess.DEVNULL,
                              stdout=stdout, stderr=stderr) as child:
            deadline = time.monotonic() + timeout
            while True:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    child.returncode = os.waitstatus_to_exitcode(status)
                    break
                if time.monotonic() > deadline:
                    child.kill()
                    raise TimeoutError(f'runtime exceeded {timeout}s')
                time.sleep(0.005)
            stdout.seek(0)
            stderr.seek(0)
            rss = usage.ru_maxrss / (1024 if sys.platform == 'darwin' else 1)
            return child.returncode, stdout.read(), stderr.read(), rss


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--target', choices=('arm64-macos', 'arm64-linux', 'x86_64-linux'))
    parser.add_argument('--runner', help='native emulator executable, e.g. qemu-aarch64')
    parser.add_argument('--case', action='append', help='run only the named case (repeatable)')
    parser.add_argument('--timeout', type=float, default=90, help='seconds per program or compile')
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error('provide a positive timeout')
    compiler = args.compiler.resolve()
    with tempfile.TemporaryDirectory(prefix='dunder-contracts-') as tmp:
        directory = Path(tmp)
        cases = {p.stem: (p, 0) for p in sorted(FIXTURES.glob('*.ddr'))}
        for name, generate in [('integer_and_float',numeric_reference),
                               ('utf8',unicode_reference), ('map_set_sort',collections_reference)]:
            source = directory / (name + '.ddr')
            source.write_text(generate(), encoding='utf-8')
            cases[name] = source, 0
        float_source, numbers = float_format_reference(directory)
        cases['float_format'] = float_source, 0
        for name, (status, body) in TRAPS.items():
            source = directory / (name + '.ddr')
            source.write_text('enum Kind{A=1,B=9}\nfn main(){print(17);' + body + ';print(99)}', encoding='utf-8')
            cases[name] = source, status
        selected = args.case or list(cases)
        if unknown := set(selected) - cases.keys():
            parser.error(f'unknown case(s): {sorted(unknown)}')
        failures = 0
        for name in selected:
            try:
                source, expected = cases[name]
                image = directory / (name + '.bin')
                command = [compiler, 'build', source, '-o', image]
                if args.target:
                    command.extend(['--target', args.target])
                built = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=args.timeout)
                if built.returncode:
                    raise ValueError(built.stderr.decode(errors='replace') or f'build exit {built.returncode}')
                run = [image]
                if args.runner:
                    run.insert(0, args.runner)
                code, output, error, rss = run_binary(run, directory, args.timeout)
                marker = b'17\n' if expected else b'42\n'
                if code == 0 and name == 'float_format':
                    check_formatted(output, numbers)
                    marker = output
                if code != expected or output != marker:
                    raise ValueError(f'exit {code}, expected {expected}; stdout {output[:500]!r}; stderr {error[:500]!r}')
                if name in BOUNDED and not args.runner and rss >= 96 * 1024:
                    raise ValueError(f'peak RSS {rss:.0f} KiB exceeds 96 MiB')
                print('PASS', name, flush=True)
            except (OSError, ValueError, OverflowError, subprocess.TimeoutExpired) as error:
                print(f'FAIL {name}: {error}', file=sys.stderr, flush=True)
                failures += 1
        print(f'{len(selected) - failures} passed, {failures} failed')
        return failures != 0


if __name__ == '__main__':
    sys.exit(main())
