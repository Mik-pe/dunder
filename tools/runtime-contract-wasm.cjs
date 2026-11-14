// Contract programs print scalars, including a marker at successful completion.
// Supports both single-image invocation and `--worker` persistent JSONL batching.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const readline = require('node:readline');

function executeOne(wasmPath, repeat, ignoreResult) {
    const mod = new WebAssembly.Module(fs.readFileSync(wasmPath));
    let repeated = false;
    let markers = 0;
    const lines = [];
    const emit = (value) => {
        lines.push(String(value) + '\n');
    };
    const { exports: program } = new WebAssembly.Instance(mod, {
        dunder: {
            print_i32(value) {
                if (repeated) {
                    assert.equal(value, 42);
                    markers++;
                } else {
                    emit(value);
                }
            },
            print_i64(value) {
                emit(value);
            },
            print_bool(value) {
                emit(Boolean(value));
            },
        },
    });
    let code = 0;
    try {
        const result = program.main();
        if (!ignoreResult && result !== undefined) code = Number(result);
        if (repeat) {
            const peak = program.memory.buffer.byteLength;
            assert.ok(peak < 96 * 1024 * 1024, 'linear memory exceeded 96 MiB');
            repeated = true;
            assert.equal(program.main(), result);
            assert.equal(markers, 1, 'second invocation must complete the same contract');
            assert.equal(program.memory.buffer.byteLength, peak, 'linear memory grew on repeated invocation');
        }
    } catch (error) {
        if (!(error instanceof WebAssembly.RuntimeError) || !/unreachable/i.test(error.message)) {
            throw error;
        }
        code = 125;
    }
    return { code, stdout: lines.join('') };
}

if (process.argv[2] === '--worker') {
    const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
    rl.on('line', (line) => {
        if (!line) return;
        let reply;
        try {
            const req = JSON.parse(line);
            const res = executeOne(req.path, Boolean(req.repeat), Boolean(req.ignoreResult));
            reply = { returncode: res.code, stdout: res.stdout, stderr: '' };
        } catch (err) {
            reply = { returncode: 1, stdout: '', stderr: String(err && err.stack ? err.stack : err) };
        }
        process.stdout.write(JSON.stringify(reply) + '\n');
    });
} else {
    const flag = process.argv[3];
    const res = executeOne(process.argv[2], flag === '--repeat', flag === '--ignore-result');
    if (res.stdout) process.stdout.write(res.stdout);
    process.exitCode = res.code;
}
