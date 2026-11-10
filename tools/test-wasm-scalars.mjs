// Execute one selfhost-generated module against JavaScript's numeric and trap semantics.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const compiler = process.argv[2];
if (!compiler) throw new Error('usage: node tools/test-wasm-scalars.mjs <dunder-compiler>');
const source = fileURLToPath(new URL('../tests/fixtures/wasm_scalars.ddr', import.meta.url));
const dir = mkdtempSync(join(tmpdir(), 'dunder-wasm-scalars-'));
try {
    const image = join(dir, 'scalars.wasm');
    execFileSync(resolve(compiler), ['build', source, '--target', 'wasm32', '-o', image], { cwd: dir });
    const module = new WebAssembly.Module(readFileSync(image));
    for (let selected = 0; selected <= 17; selected++) {
        const printed = [];
        const print = value => printed.push(value);
        const { exports } = new WebAssembly.Instance(module, {
            oracle: { case_id: () => selected },
            dunder: {
                print_i32: print, print_i64: print, print_u32: value => print(value >>> 0),
                print_u64: value => print(BigInt.asUintN(64, value)), print_f32: print, print_f64: print,
            },
        });
        if (selected === 0) {
            assert.equal(exports.main(), 42);
            assert.deepEqual(printed, [4294967295, 18446744073709551615n, -0, -2.25, Number.MIN_VALUE]);
        } else {
            assert.throws(() => exports.main(), WebAssembly.RuntimeError, 'expected trap in case ' + selected);
            assert.deepEqual(printed, [], 'arithmetic must trap before print in case ' + selected);
        }
    }
    console.log('wasm32 scalar boundaries, packed memory, typed print and 17 traps passed');
} finally {
    rmSync(dir, { recursive: true, force: true });
}
