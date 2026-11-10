// Contract programs print integers, including a marker at successful completion.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const mod = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
let repeated = false;
let markers = 0;
const { exports: program } = new WebAssembly.Instance(mod, {
    dunder: {
        print_i32(value) {
            if (repeated) {
                assert.equal(value, 42);
                markers++;
            } else {
                console.log(value);
            }
        },
    },
});
try {
    const result = program.main();
    if (result !== undefined) process.exitCode = Number(result);
    if (process.argv[3] === '--repeat') {
        const peak = program.memory.buffer.byteLength;
        assert.ok(peak < 96 * 1024 * 1024, 'linear memory exceeded 96 MiB');
        repeated = true;
        assert.equal(program.main(), result);
        assert.equal(markers, 1, 'second invocation must complete the same contract');
        assert.equal(program.memory.buffer.byteLength, peak, 'linear memory grew on repeated invocation');
    }
} catch (error) {
    if (!(error instanceof WebAssembly.RuntimeError) || error.message !== 'unreachable') throw error;
    process.exitCode = 125;
}
