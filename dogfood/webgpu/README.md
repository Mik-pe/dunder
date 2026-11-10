# WebGPU triangle

Metal, Vulkan and this WebGPU host use the same Dunder vertex data from
`dogfood/graphics/triangle.ddr`. The Wasm program calls the host with a
surface ID and a pointer into its exported memory. The browser copies those
vertices into a WebGPU buffer and presents the triangle.

From the repository root, run

```sh
mkdir -p build/webgpu
dunder build dogfood/webgpu/main.ddr --target wasm32 -o build/webgpu/triangle.wasm
cp dogfood/webgpu/index.html dogfood/webgpu/host.js build/webgpu/
python3 -m http.server 8000 --bind 127.0.0.1 --directory build/webgpu
```

Open [the local demo](http://localhost:8000) in a WebGPU-capable browser.
