#!/usr/bin/env python3
"""Exercise std.math in native and Wasm images against the host libm."""

import math
from pathlib import Path
import platform
import random
import struct
import subprocess
import sys
import tempfile


NATIVE = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
TARGETS = (*NATIVE, 'wasm32')
WASM = """
const binary = require('node:fs').readFileSync(process.argv[1]);
const image = new WebAssembly.Module(binary);
const instance = new WebAssembly.Instance(image, {dunder: {
    print_i64: value => console.log(value.toString())
}});
instance.exports.main();
"""


def bits(value):
    return struct.unpack('<q', struct.pack('<d', value))[0]


def float64(word):
    return struct.unpack('<d', struct.pack('<q', word))[0]


def angles():
    rng = random.Random(0x73a8c9)
    values = [0.0, -0.0, math.ulp(0.0), -math.ulp(0.0), sys.float_info.max, -sys.float_info.max]
    for exponent in range(2047):
        word = (exponent << 52) | rng.getrandbits(52)
        if exponent % 2:
            word -= 1 << 63
        values.append(float64(word))
    for multiple in (0.5, 1, 2, 3, 4, 17, 8192, 333772, 524288, 2**30, 2**53, 2**100, 2**500):
        pole = math.pi * 0.5 * multiple
        values.extend((math.nextafter(pole, -math.inf), pole, math.nextafter(pole, math.inf), -pole))
    for endpoint in (math.pi / 4, 524288.0, 1e20, 1e100, 1e300):
        values.extend((math.nextafter(endpoint, -math.inf), endpoint, math.nextafter(endpoint, math.inf)))
    return values


def program(values):
    words = ',\n        '.join(str(bits(value)) for value in values)
    return f"""import std.math as math
fn main() {{
    nan := f64.from_bits(0x7ff8_0000_0000_002a)
    inf := f64.from_bits(0x7ff0_0000_0000_0000)
    assert(math.min(4.0, 7.0) == 4.0 && math.max(4.0, 7.0) == 7.0)
    assert(math.min(0.0, -0.0).is_sign_negative())
    assert(math.min(-0.0, 0.0).is_sign_negative())
    assert(!math.max(-0.0, 0.0).is_sign_negative())
    assert(!math.max(0.0, -0.0).is_sign_negative())
    assert(math.min(inf, 0.0) == 0.0 && math.max(-inf, 0.0) == 0.0)
    assert(math.min(1.0, nan).to_bits() == nan.to_bits())
    assert(math.max(nan, 1.0).to_bits() == nan.to_bits())
    assert(math.clamp(-4.0, 0.0, 1.0) == 0.0)
    assert(math.clamp(0.5, 0.0, 1.0) == 0.5 && math.clamp(3.0, 0.0, 1.0) == 1.0)
    assert(math.clamp(0.5, 1.0, 0.0).is_nan() && math.clamp(0.5, nan, 1.0).is_nan())
    assert(math.clamp(0.5, 0.0, nan).is_nan() && math.clamp(nan, 0.0, 1.0).is_nan())
    assert(math.clamp(-0.0, 0.0, 1.0).is_sign_negative())
    assert(math.sin(-0.0).is_sign_negative() && math.tan(-0.0).is_sign_negative() && math.cos(-0.0) == 1.0)
    assert(math.sin(inf).is_nan() && math.cos(-inf).is_nan() && math.tan(inf).is_nan())
    assert(math.sin(nan).is_nan() && math.cos(nan).is_nan() && math.tan(nan).is_nan())
    angles: [i64; {len(values)}] = [
        {words},
    ]
    for word in angles {{
        angle := f64.from_bits(word)
        print(math.sin(angle).to_bits())
        print(math.cos(angle).to_bits())
        print(math.tan(angle).to_bits())
    }}
}}
"""


def run(command, work):
    result = subprocess.run(command, cwd=work, capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, (command, result.returncode, result.stdout[-240:], result.stderr)
    return result.stdout


def validate(output, values, target, mode):
    results = [int(line) for line in output.splitlines()]
    assert len(results) == 3 * len(values), (target, mode, len(results))
    functions = (math.sin, math.cos, math.tan)
    maxima = [0, 0, 0]
    for index, angle in enumerate(values):
        for column, function in enumerate(functions):
            expected = bits(function(angle))
            actual = results[3 * index + column]
            # Equal signs make the distance between encodings the distance in ULPs.
            distance = abs(actual - expected)
            assert distance <= 8, (target, mode, function.__name__, angle, distance, actual, expected)
            maxima[column] = max(maxima[column], distance)
    return maxima


def main():
    compiler = Path(sys.argv[1]).resolve(strict=True)
    host = 'arm64-macos' if platform.system() == 'Darwin' else (
        'x86_64-linux' if platform.machine() == 'x86_64' else 'arm64-linux')
    values = angles()
    with tempfile.TemporaryDirectory(prefix='dunder-math-') as temporary:
        work = Path(temporary)
        source = work / 'math_cases.ddr'
        source.write_text(program(values))
        for target in TARGETS:
            for mode, flags in (('default', []), ('direct', ['--mir=off', '--scalar-opt=off'])):
                image = work / target
                run([compiler, 'build', source, '--target', target, *flags, '-o', image], work)
                if target == host:
                    output = run([image], work)
                elif target == 'wasm32':
                    output = run(['node', '-e', WASM, image], work)
                else:
                    continue
                errors = validate(output, values, target, mode)
                print(f'{target} {mode}: {len(values)} angles, max ULP for sin/cos/tan {errors}')


if __name__ == '__main__':
    main()
