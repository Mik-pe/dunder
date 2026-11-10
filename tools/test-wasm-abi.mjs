// Exercise compiler-produced Wasm modules against the JS host ABI.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const compiler = resolve(process.argv[2]);
const root = fileURLToPath(new URL('../', import.meta.url));
const work = mkdtempSync(join(tmpdir(), 'dunder-wasm-abi-'));
function compile(source) {
    const output = join(work, 'module.wasm');
    execFileSync(compiler, ['build', source, '--target', 'wasm32', '-o', output], { cwd: root });
    return new WebAssembly.Module(readFileSync(output));
}
try {
    const mod = compile('tests/fixtures/wasm_abi.ddr');
    assert.deepEqual(WebAssembly.Module.imports(mod).map(x => `${x.module}.${x.name}`).sort(), ['test.bridge', 'test.inspect']);
    let instance, lateCallback;
    instance = new WebAssembly.Instance(mod, { test: {
        inspect(p, size, align) {
            const mem = new DataView(instance.exports.memory.buffer);
            assert.equal(size, 24); assert.equal(align, 8); assert.equal(p % align, 0);
            assert.equal(mem.getUint8(p), 251);
            const text = mem.getUint32(p + 4, true);
            assert.deepEqual([...new Uint8Array(mem.buffer, text, 6)], [104,101,108,108,111,0]);
            assert.equal(mem.getUint32(p + 8, true), 5);
            lateCallback = instance.exports.__indirect_function_table.get(mem.getUint32(p + 12, true));
            assert.equal(lateCallback(41), 42);
            assert.equal(mem.getBigUint64(p + 16, true), 1234567890123456789n);
            mem.setUint32(p + 8, 1234, true);
            return 42;
        },
        bridge(cb, ...args) {
            assert.deepEqual(args.slice(0, 7), [-7, 1.25, -0.5, -4294967297n, 255, 65000, 4000000000 | 0]);
            assert.deepEqual(args.slice(8), [-4096, -1n]);
            return instance.exports.__indirect_function_table.get(cb)(...args);
        },
    } });
    assert.equal(instance.exports.main(), 73); assert.equal(lateCallback(98), 99);
    const scene = compile('dogfood/webgpu/main.ddr');
    let draws = 0;
    instance = new WebAssembly.Instance(scene, { webgpu: { draw(surface, p, count) {
        assert.equal(surface, 1); assert.equal(count, 3);
        const vertices = [0, .72, 0, 1, 1, .69, .28, 1, -.72, -.62, 0, 1, .16, .87, .74, 1, .72, -.62, 0, 1, .40, .47, 1, 1];
        assert.deepEqual([...new Float32Array(instance.exports.memory.buffer, p, vertices.length)], vertices.map(Math.fround));
        draws++;
    } } });
    instance.exports.main(); assert.equal(draws, 1);
    console.log('Wasm C layout, typed host calls, callbacks and WebGPU vertex data passed');
} finally { rmSync(work, { recursive: true, force: true }); }
