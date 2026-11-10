async function render(canvas) {
    const adapter = await navigator.gpu?.requestAdapter();
    if (!adapter) throw new Error("WebGPU is unavailable on this device.");
    const device = await adapter.requestDevice();
    const context = canvas.getContext("webgpu");
    const format = navigator.gpu.getPreferredCanvasFormat();
    context.configure({ device, format });
    const shader = device.createShaderModule({ code: `
        struct Vertex {
            @builtin(position) position: vec4f,
            @location(0) color: vec4f,
        }
        @vertex fn vertex(@location(0) position: vec4f, @location(1) color: vec4f) -> Vertex {
            return Vertex(position, color);
        }
        @fragment fn fragment(v: Vertex) -> @location(0) vec4f { return v.color; }
    ` });
    const pipeline = await device.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module: shader, entryPoint: "vertex", buffers: [{
            arrayStride: 32, attributes: [
                { shaderLocation: 0, offset: 0, format: "float32x4" },
                { shaderLocation: 1, offset: 16, format: "float32x4" },
            ],
        }] },
        fragment: { module: shader, entryPoint: "fragment", targets: [{ format }] },
        primitive: { topology: "triangle-list" },
    });
    function draw(surface, address, count) {
        if (surface !== 1 || count !== 3) throw new Error("Invalid triangle submission.");
        const vertices = new Uint8Array(instance.exports.memory.buffer, address >>> 0, count * 32);
        const buffer = device.createBuffer({ size: vertices.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
        device.queue.writeBuffer(buffer, 0, vertices);
        const commands = device.createCommandEncoder();
        const pass = commands.beginRenderPass({ colorAttachments: [{
            view: context.getCurrentTexture().createView(),
            clearValue: [0.035, 0.045, 0.075, 1], loadOp: "clear", storeOp: "store",
        }] });
        pass.setPipeline(pipeline);
        pass.setVertexBuffer(0, buffer);
        pass.draw(count);
        pass.end();
        device.queue.submit([commands.finish()]);
        buffer.destroy();
    }
    const { instance } = await WebAssembly.instantiateStreaming(fetch("triangle.wasm"), { webgpu: { draw } });
    device.pushErrorScope("validation");
    instance.exports.main();
    const error = await device.popErrorScope();
    if (error) throw new Error(error.message);
    await device.queue.onSubmittedWorkDone();
}
render(document.querySelector("canvas")).catch(error => {
    document.querySelector("output").textContent = error.message;
});
