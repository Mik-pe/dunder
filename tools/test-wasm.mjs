// Run compiler-produced @test images in separate Node processes, one entry per group.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(import.meta.url);
const root = fileURLToPath(new URL('../', import.meta.url));

// These dogfood contracts use process or filesystem services. A target that
// learns to compile them still runs their assertions.
const nativeGroups = new Set([
    'dogfood/filesystem.ddr::files_round_trip_through_the_os',
    'dogfood/filesystem.ddr::errors_remain_values',
    'dogfood/enums.ddr::value_enums_work_in_records_and_arrays',
    'selfhost/compile.ddr::string_operand_leases_are_removed_only_across_effect_free_rhs',
    'selfhost/compile.ddr::ownership_proofs_do_not_change_validation_or_binding_records',
    'selfhost/compile.ddr::rune_literals_emit_the_same_code_as_scalar_constants',
    'selfhost/compile.ddr::compiler_checks_types_before_emitting_an_image',
    'selfhost/compile.ddr::native_compiler_rejects_unsupported_aggregate_shapes_and_immutable_mutation',
]);

function invoke(program, args) {
    const result = spawnSync(program, args, { cwd: root, encoding: 'utf8', timeout: 60000 });
    if (result.error) throw result.error;
    if (result.signal) throw Error(`${program} terminated with ${result.signal}`);
    return result;
}

async function execute(path) {
    const module = await WebAssembly.compile(readFileSync(path));
    let instance;
    const print = x => console.log(x.toString());
    instance = new WebAssembly.Instance(module, { dunder: {
        print_i32: print, print_i64: print, print_f32: print, print_f64: print,
        print_u32: x => print(x >>> 0), print_u64: x => print(BigInt.asUintN(64, x)),
        print_bool: x => print(Boolean(x)), print_rune: x => print(String.fromCodePoint(x)),
        print_string(p) {
            const memory = instance.exports.memory.buffer;
            const n = new DataView(memory).getUint32(p, true);
            print(new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(memory, p + 8, n)));
        },
    } });
    assert.equal(instance.exports.main(), 0);
}

function suite(compiler, source) {
    const listing = invoke(compiler, ['build-tests', source, '--list']);
    if (listing.status !== 0) throw Error(listing.stderr);
    const groups = listing.stdout.trimEnd().split('\n').filter(Boolean);
    assert.ok(groups.length, 'suite contains no test groups');
    const dir = mkdtempSync(join(tmpdir(), 'dunder-wasm-tests-'));
    let passed = 0, failed = 0, native = 0;
    try {
        for (const [id, line] of groups.entries()) {
            assert.ok(line.startsWith(`${id} `), `invalid group listing: ${line}`);
            const name = line.slice(String(id).length + 1);
            const image = join(dir, `${id}.wasm`);
            const build = invoke(compiler, ['build-tests', source, '--target', 'wasm32', '--group', String(id), '-o', image]);
            if (build.status !== 0 && nativeGroups.has(name) && /native system call|requires a native target|requires native|native target required/i.test(build.stderr)) {
                console.log(`NATIVE ${name}`); native++;
                continue;
            }
            const result = build.status === 0 ? invoke(process.execPath, [script, '--execute', image]) : build;
            if (result.status === 0) {
                console.log(`PASS ${name}`); passed++;
            } else {
                console.error(`FAIL ${name}\n${result.stderr}${result.stdout}`); failed++;
            }
        }
    } finally {
        rmSync(dir, { recursive: true, force: true });
    }
    console.log(`${passed} passed, ${failed} failed, ${native} native-only`);
    process.exitCode = failed > 0 ? 1 : 0;
}

const [, , compiler, source] = process.argv;
if (compiler === '--execute' && source) await execute(source);
else if (compiler && process.argv.length <= 4) suite(resolve(compiler), source || 'dogfood/tests.ddr');
else throw Error('usage: node tools/test-wasm.mjs /path/to/dunder [suite.ddr]');
