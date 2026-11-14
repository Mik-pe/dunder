// Run compiler-produced @test images in isolated WebAssembly instances, one entry per group.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { availableParallelism, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(import.meta.url);
const root = fileURLToPath(new URL('../', import.meta.url));

// These native contracts use process, socket, or filesystem services. A target that
// learns to compile them still runs their assertions.
const nativeGroups = new Set([
    'tests/net_loopback.ddr::clock_and_confined_dir_bytes',
    'tests/net_loopback.ddr::loopback_nonblocking_poll_roundtrip',
    'tests/net_loopback.ddr::stream_set_pool_and_slot_reuse',
    'tests/net_loopback.ddr::files_round_trip_through_the_os',
    'tests/net_loopback.ddr::errors_remain_values',
    'tests/process_fs.ddr::fs_temp_dir_links_listing_and_safe_remove_all',
    'tests/process_fs.ddr::process_grant_and_env_cwd_stderr_exit_code',
    'tests/process_fs.ddr::process_large_simultaneous_stdin_stdout_pipe_without_deadlock',
    'tests/process_fs.ddr::process_launch_failures_report_portable_errno',
    'tests/process_fs.ddr::process_timeout_and_background_child_spawn',
    'selfhost/compile.ddr::string_operand_leases_are_removed_only_across_effect_free_rhs',
    'selfhost/compile.ddr::ownership_proofs_do_not_change_validation_or_binding_records',
    'selfhost/compile.ddr::rune_literals_emit_the_same_code_as_scalar_constants',
    'selfhost/compile.ddr::compiler_checks_types_before_emitting_an_image',
    'selfhost/compile.ddr::native_compiler_rejects_unsupported_aggregate_shapes_and_immutable_mutation',
    'selfhost/compile.ddr::target_is_folds_at_compile_time_and_prunes_dead_extern_branches',
    'selfhost/compile.ddr::inline_producer_preserves_authoritative_diagnostics_and_scalar_off_behavior',
    'selfhost/compile.ddr::inline_producer_does_not_duplicate_source_binding_reports',
    'selfhost/compile.ddr::inline_fragments_restore_backward_dependencies_charges_and_private_body_identity',
    'selfhost/compile.ddr::inline_fragment_keys_preserve_cold_growth_frontiers_across_partial_cache_hits',
    'selfhost/compile.ddr::all_emitted_source_functions_use_mir_without_changing_diagnostics_or_bindings',
    'selfhost/compile.ddr::whole_function_optimizer_policy_and_actual_counters_follow_pass_execution',
]);

function invoke(program, args) {
    const result = spawnSync(program, args, { cwd: root, encoding: 'utf8', timeout: 60000 });
    if (result.error) throw result.error;
    if (result.signal) throw Error(`${program} terminated with ${result.signal}`);
    return result;
}

function invokeAsync(program, args) {
    return new Promise((resolvePromise, rejectPromise) => {
        const child = spawn(program, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
        let stdout = '';
        let stderr = '';
        child.stdout.setEncoding('utf8');
        child.stderr.setEncoding('utf8');
        child.stdout.on('data', chunk => { stdout += chunk; });
        child.stderr.on('data', chunk => { stderr += chunk; });
        child.on('error', rejectPromise);
        child.on('close', (status, signal) => {
            if (signal) rejectPromise(Error(`${program} terminated with ${signal}`));
            else resolvePromise({ status: status ?? 1, stdout, stderr });
        });
    });
}

async function execute(path, sink = line => console.log(line)) {
    const module = await WebAssembly.compile(readFileSync(path));
    let instance;
    const print = x => sink(x.toString());
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

async function suite(compiler, source) {
    const listing = invoke(compiler, ['build-tests', source, '--list']);
    if (listing.status !== 0) throw Error(listing.stderr);
    const groups = listing.stdout.trimEnd().split('\n').filter(Boolean);
    assert.ok(groups.length, 'suite contains no test groups');
    const dir = mkdtempSync(join(tmpdir(), 'dunder-wasm-tests-'));
    const outcomes = new Array(groups.length);
    let nextId = 0;
    const workers = Math.min(groups.length, Math.max(1, availableParallelism()));
    try {
        await Promise.all(Array.from({ length: workers }, async () => {
            while (nextId < groups.length) {
                const id = nextId++;
                const line = groups[id];
                assert.ok(line.startsWith(`${id} `), `invalid group listing: ${line}`);
                const name = line.slice(String(id).length + 1);
                const image = join(dir, `${id}.wasm`);
                const build = await invokeAsync(compiler, ['build-tests', source, '--target', 'wasm32', '--group', String(id), '--jobs', '1', '-o', image]);
                if (build.status !== 0 && nativeGroups.has(name) && /native system call|requires a native target|requires the selected native platform|requires native|native target required/i.test(build.stderr)) {
                    outcomes[id] = { kind: 'native', name };
                    continue;
                }
                if (build.status !== 0) {
                    outcomes[id] = { kind: 'fail', name, detail: `${build.stderr}${build.stdout}` };
                    continue;
                }
                const logs = [];
                try {
                    await execute(image, msg => logs.push(msg));
                    outcomes[id] = { kind: 'pass', name };
                } catch (err) {
                    outcomes[id] = { kind: 'fail', name, detail: `${logs.join('\n')}${logs.length ? '\n' : ''}${err?.stack || err}` };
                }
            }
        }));
    } finally {
        rmSync(dir, { recursive: true, force: true });
    }
    let passed = 0, failed = 0, native = 0;
    for (const item of outcomes) {
        if (item.kind === 'native') {
            console.log(`NATIVE ${item.name}`); native++;
        } else if (item.kind === 'pass') {
            console.log(`PASS ${item.name}`); passed++;
        } else {
            console.error(`FAIL ${item.name}\n${item.detail}`); failed++;
        }
    }
    console.log(`${passed} passed, ${failed} failed, ${native} native-only`);
    process.exitCode = failed > 0 ? 1 : 0;
}

const [, , compiler, source] = process.argv;
if (compiler === '--execute' && source) await execute(source);
else if (compiler && process.argv.length <= 4) await suite(resolve(compiler), source || 'tests/suite.ddr');
else throw Error('usage: bun tools/test-wasm.mjs /path/to/dunder [suite.ddr]');
