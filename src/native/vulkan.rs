//! Vulkan 1.0 graphics emitted directly into native Linux executables.
//! Vulkan owns rasterization; SDL's software surface only presents readback pixels.
use super::{Branch, Code, Label};
use crate::{
    check::{Expr, VulkanOp},
    syntax::Type,
};
mod pipeline;
mod shaders;
mod window;

const FRAME: usize = 512;
const GEN: u32 = 8;
const ACTIVE: u32 = 16;
const HEADLESS: u32 = 24;
const WIDTH: u32 = 32;
const HEIGHT: u32 = 40;
const FRAMES: u32 = 48;
const INSTANCE: u32 = 56;
const PHYSICAL: u32 = 64;
const DEVICE: u32 = 72;
const QUEUE: u32 = 80;
const FAMILY: u32 = 88;
const COMMAND_POOL: u32 = 96;
const COMMAND: u32 = 104;
const IMAGE: u32 = 112;
const IMAGE_MEMORY: u32 = 120;
const IMAGE_VIEW: u32 = 128;
const RENDER_PASS: u32 = 136;
const PIPELINE_LAYOUT: u32 = 144;
const PIPELINE: u32 = 152;
const FRAMEBUFFER: u32 = 160;
const VERTEX_BUFFER: u32 = 168;
const VERTEX_MEMORY: u32 = 176;
const VERTEX_MAP: u32 = 184;
const READ_BUFFER: u32 = 192;
const READ_MEMORY: u32 = 200;
const PIXELS: u32 = 208;
const VERTEX_SHADER: u32 = 216;
const FRAGMENT_SHADER: u32 = 224;
const PIXEL_BYTES: u32 = 232;
const CLOSED: u32 = 256;
const POINTER_X: u32 = 264;
const POINTER_Y: u32 = 272;
const POINTER_DOWN: u32 = 280;
const KEY: u32 = 288;
const RESIZED: u32 = 296;
const TIME: u32 = 304;
const WINDOW_WIDTH: u32 = 312;
const WINDOW_HEIGHT: u32 = 320;
const ERROR_CODE: u32 = 328;
const FIRST_POLL: u32 = 376;
const NAME: u32 = 512;
const MEMORY_PROPERTIES: u32 = 8192;
const SCRATCH: u32 = 24576;
const VERTEX_BYTES: u64 = 100_000 * 32;

const FUNCTIONS: &[&str] = &[
    "vkCreateInstance",
    "vkEnumeratePhysicalDevices",
    "vkGetPhysicalDeviceProperties",
    "vkGetPhysicalDeviceQueueFamilyProperties",
    "vkGetPhysicalDeviceMemoryProperties",
    "vkCreateDevice",
    "vkGetDeviceQueue",
    "vkCreateCommandPool",
    "vkAllocateCommandBuffers",
    "vkCreateImage",
    "vkGetImageMemoryRequirements",
    "vkAllocateMemory",
    "vkBindImageMemory",
    "vkCreateImageView",
    "vkCreateRenderPass",
    "vkCreateFramebuffer",
    "vkCreateShaderModule",
    "vkCreatePipelineLayout",
    "vkCreateGraphicsPipelines",
    "vkCreateBuffer",
    "vkGetBufferMemoryRequirements",
    "vkBindBufferMemory",
    "vkMapMemory",
    "vkResetCommandBuffer",
    "vkBeginCommandBuffer",
    "vkCmdBeginRenderPass",
    "vkCmdBindPipeline",
    "vkCmdBindVertexBuffers",
    "vkCmdDraw",
    "vkCmdEndRenderPass",
    "vkCmdCopyImageToBuffer",
    "vkEndCommandBuffer",
    "vkQueueSubmit",
    "vkQueueWaitIdle",
    "vkDeviceWaitIdle",
    "vkDestroyBuffer",
    "vkFreeMemory",
    "vkDestroyFramebuffer",
    "vkDestroyPipeline",
    "vkDestroyPipelineLayout",
    "vkDestroyShaderModule",
    "vkDestroyRenderPass",
    "vkDestroyImageView",
    "vkDestroyImage",
    "vkDestroyCommandPool",
    "vkDestroyDevice",
    "vkDestroyInstance",
    "vkUnmapMemory",
    "vkCmdPipelineBarrier",
    "clock_gettime",
];
#[derive(Clone, Copy)]
enum Arg {
    Ctx(u32),
    Slot(usize),
    Imm(u64),
    Addr(u32),
}
#[derive(Default)]
pub(super) struct Runtime {
    pub calls: [Label; 7],
    init: Label,
    validate: Label,
    setup: Label,
    render: Label,
    destroy: Label,
    memory_type: Label,
    buffer: Label,
    window_open: Label,
    window_poll: Label,
    window_present: Label,
    window_close: Label,
}
impl Runtime {
    pub fn new(c: &mut Code<'_>) -> Self {
        Self {
            calls: std::array::from_fn(|_| c.label()),
            init: c.label(),
            validate: c.label(),
            setup: c.label(),
            render: c.label(),
            destroy: c.label(),
            memory_type: c.label(),
            buffer: c.label(),
            window_open: c.label(),
            window_poll: c.label(),
            window_present: c.label(),
            window_close: c.label(),
        }
    }
}
fn index(op: VulkanOp) -> usize {
    match op {
        VulkanOp::Open => 0,
        VulkanOp::OpenHeadless => 1,
        VulkanOp::Poll => 2,
        VulkanOp::Draw => 3,
        VulkanOp::Close => 4,
        VulkanOp::DeviceName => 5,
        VulkanOp::Pixels => 6,
    }
}
impl Code<'_> {
    pub(super) fn vulkan_expr(&mut self, op: VulkanOp, args: &[Expr], storage: Option<usize>) {
        for arg in args {
            self.expr(arg);
            self.push(0);
        }
        for reg in (0..args.len()).rev() {
            self.pop(reg as u32);
        }
        if let Some(slot) = storage {
            self.local_addr(slot);
            self.mov(8, 9);
        }
        self.branch(self.vulkan.calls[index(op)], Branch::Call);
        if let Some(slot) = storage {
            self.local_addr(slot);
            self.mov(0, 9);
        }
    }
    fn vk_ctx(&mut self, reg: u32) {
        self.global_addr(2, reg);
        self.ldr(reg, reg);
    }
    fn vk_addr(&mut self, off: u32, reg: u32) {
        self.vk_ctx(reg);
        self.imm(17, off as u64);
        self.add(reg, reg, 17);
    }
    fn vk_get(&mut self, off: u32, reg: u32) {
        self.vk_addr(off, 9);
        self.ldr(reg, 9);
    }
    fn vk_set(&mut self, off: u32, reg: u32) {
        assert_ne!(reg, 9);
        self.vk_addr(off, 9);
        self.str(reg, 9);
    }
    fn vk_imm(&mut self, off: u32, n: u64) {
        self.imm(0, n);
        self.vk_set(off, 0);
    }
    fn vk_u32(&mut self, off: u32, n: u32) {
        self.imm(0, n as u64);
        self.vk_addr(off, 9);
        self.typed_store_reg(Type::I32, 9, 0);
    }
    fn vk_read32(&mut self, off: u32, reg: u32) {
        self.vk_addr(off, 9);
        self.emit(0xb9400000 | (9 << 5) | reg);
    }
    fn vk_write32(&mut self, off: u32, reg: u32) {
        self.vk_addr(off, 9);
        self.typed_store_reg(Type::I32, 9, reg);
    }
    fn vk_pointer(&mut self, off: u32, target: u32) {
        self.vk_addr(target, 0);
        self.vk_set(off, 0);
    }
    fn vk_zero(&mut self, off: u32, len: u32) {
        self.vk_addr(off, 10);
        self.imm(11, len as u64);
        let again = self.label();
        self.mark(again);
        self.strb(31, 10);
        self.add_imm(10, 10, 1);
        self.sub_imm(11, 11, 1);
        self.branch(again, Branch::Nonzero(11));
    }
    fn vk_cstr(&mut self, text: &str, reg: u32) {
        let label = self.string_literal(&format!("{text}\0"));
        self.branch(label, Branch::Address(reg));
        self.add_imm(reg, reg, 8);
    }
    fn vk_import(&mut self, index: usize, argc: u32) {
        self.global_addr(index, 16);
        self.ldr(16, 16);
        self.native_call(16, argc, 0);
    }
    fn vk_call(&mut self, name: &str, args: &[Arg]) {
        let offset = if let Some(i) = FUNCTIONS.iter().position(|n| *n == name) {
            1024 + i as u32 * 8
        } else {
            window::SDL_FUNCTIONS
                .iter()
                .find(|(n, _)| *n == name)
                .expect("unknown native function")
                .1
        };
        // Resolve all addresses before assigning argument registers: helpers use x9/x17.
        for (i, arg) in args.iter().enumerate() {
            match arg {
                Arg::Ctx(off) => self.vk_get(*off, 0),
                Arg::Slot(slot) => self.load(*slot),
                Arg::Imm(n) => self.imm(0, *n),
                Arg::Addr(off) => self.vk_addr(*off, 0),
            }
            self.push(0);
            if i >= 15 {
                panic!("too many native arguments");
            }
        }
        self.vk_get(offset, 16);
        for i in (0..args.len()).rev() {
            self.pop(i as u32);
        }
        self.native_call(16, args.len() as u32, 0);
    }
    fn vk_load_functions(&mut self, library: &str, functions: &[(&str, u32)], failed: Label) {
        self.vk_cstr(library, 0);
        self.imm(1, 2);
        self.vk_import(0, 2);
        self.branch(failed, Branch::Zero(0));
        self.store(50, 0);
        for (name, offset) in functions {
            self.load(50);
            self.vk_cstr(name, 1);
            self.vk_import(1, 2);
            self.branch(failed, Branch::Zero(0));
            self.vk_set(*offset, 0);
        }
        self.load(50);
    }
    fn vk_result(&mut self, failed: Label) {
        self.vk_set(ERROR_CODE, 0);
        self.branch(failed, Branch::Nonzero(0));
    }
    fn vk_clock(&mut self) {
        // A monotonic clock also makes headless animation advance between frames.
        self.vk_zero(SCRATCH + 4096, 16);
        self.vk_call("clock_gettime", &[Arg::Imm(1), Arg::Addr(SCRATCH + 4096)]);
        self.vk_get(SCRATCH + 4096, 0);
        self.imm(1, 1000);
        self.emit(0x9b017c00);
        self.vk_get(SCRATCH + 4104, 1);
        self.imm(2, 1_000_000);
        self.emit(0x9ac20821);
        self.add(0, 0, 1);
    }
    fn vk_type(&self, op: VulkanOp) -> Type {
        let t = self.vulkan_types.unwrap();
        match op {
            VulkanOp::Open | VulkanOp::OpenHeadless => t.open,
            VulkanOp::Poll => t.frame,
            VulkanOp::Draw => t.draw,
            _ => Type::Unit,
        }
    }
    fn vk_begin(&mut self, op: VulkanOp) -> Type {
        self.mark(self.vulkan.calls[index(op)]);
        self.prologue(FRAME);
        self.store(0, 8);
        for reg in 0..5 {
            self.store(reg as usize + 1, reg);
        }
        let ty = self.vk_type(op);
        if let Some(bytes) = self.types.storage_bytes(ty) {
            self.load(0);
            self.mov(10, 0);
            self.imm(11, bytes as u64);
            let zero = self.label();
            self.mark(zero);
            self.strb(31, 10);
            self.add_imm(10, 10, 1);
            self.sub_imm(11, 11, 1);
            self.branch(zero, Branch::Nonzero(11));
        }
        if matches!(op, VulkanOp::Open | VulkanOp::OpenHeadless | VulkanOp::Draw) {
            let empty = self.string_literal("");
            self.branch(empty, Branch::Address(0));
            self.vk_field(ty, "error", 0);
        }
        ty
    }
    fn vk_field(&mut self, mut ty: Type, path: &str, reg: u32) {
        let mut offset = 0;
        for name in path.split('.') {
            let f = self
                .types
                .record(ty)
                .unwrap()
                .fields
                .iter()
                .find(|f| f.name == name)
                .unwrap();
            offset += f.offset;
            ty = f.ty;
        }
        self.local_addr(0);
        self.ldr(9, 9);
        self.imm(10, offset as u64);
        self.add(9, 9, 10);
        self.typed_store_reg(ty, 9, reg);
    }
    fn vk_return(&mut self) {
        self.load(0);
        self.epilogue();
    }
    fn vk_ok(&mut self, ty: Type) {
        self.imm(0, 1);
        self.vk_field(ty, "ok", 0);
        self.vk_return();
    }
    fn vk_error(&mut self, ty: Type, message: &str) {
        let s = self.string_literal(message);
        self.branch(s, Branch::Address(0));
        self.vk_field(ty, "error", 0);
        self.imm(0, 0);
        self.vk_field(ty, "ok", 0);
        self.vk_return();
    }
    pub(super) fn vulkan_runtime(&mut self) {
        self.vk_init_runtime();
        self.vk_setup_runtime();
        self.vk_render_runtime();
        self.vk_destroy_runtime();
        self.vk_window_runtime();
        self.vk_api_runtime();
    }
    fn vk_init_runtime(&mut self) {
        self.mark(self.vulkan.init);
        self.prologue(FRAME);
        let mapped = self.label();
        let ready = self.label();
        let failed = self.label();
        self.vk_ctx(0);
        self.branch(mapped, Branch::Nonzero(0));
        self.imm(0, 0);
        self.imm(1, 65536);
        self.imm(2, 3);
        self.imm(3, 0x22);
        self.imm(4, u64::MAX);
        self.imm(5, 0);
        self.syscall(197);
        self.branch(failed, Branch::Cond(2));
        self.global_addr(2, 9);
        self.str(0, 9);
        self.mark(mapped);
        self.vk_get(0, 0);
        self.branch(ready, Branch::Nonzero(0));
        let functions: Vec<_> = FUNCTIONS
            .iter()
            .enumerate()
            .map(|(i, n)| (*n, 1024 + i as u32 * 8))
            .collect();
        self.vk_load_functions("libvulkan.so.1", &functions, failed);
        self.vk_set(0, 0);
        self.mark(ready);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
        self.mark(self.vulkan.validate);
        self.mov(1, 0);
        self.vk_ctx(0);
        let invalid = self.label();
        self.branch(invalid, Branch::Zero(0));
        self.vk_get(ACTIVE, 2);
        self.branch(invalid, Branch::Zero(2));
        self.vk_get(GEN, 2);
        self.cmp(1, 2);
        self.branch(invalid, Branch::Cond(1));
        self.imm(0, 1);
        self.ret();
        self.mark(invalid);
        self.imm(0, 0);
        self.ret();
    }
    fn vk_api_runtime(&mut self) {
        for op in [VulkanOp::Open, VulkanOp::OpenHeadless] {
            let ty = self.vk_begin(op);
            let dimensions = self.label();
            let unavailable = self.label();
            let occupied = self.label();
            let failed = self.label();
            let width = if matches!(op, VulkanOp::Open) { 2 } else { 1 };
            let height = width + 1;
            for slot in [width, height] {
                self.load(slot);
                self.imm(1, 1);
                self.cmp(0, 1);
                self.branch(dimensions, Branch::Cond(11));
                self.imm(1, 4096);
                self.cmp(0, 1);
                self.branch(dimensions, Branch::Cond(12));
            }
            self.branch(self.vulkan.init, Branch::Call);
            self.branch(unavailable, Branch::Zero(0));
            self.vk_get(ACTIVE, 0);
            self.branch(occupied, Branch::Nonzero(0));
            self.load(width);
            self.vk_set(WIDTH, 0);
            self.vk_set(WINDOW_WIDTH, 0);
            self.load(height);
            self.vk_set(HEIGHT, 0);
            self.vk_set(WINDOW_HEIGHT, 0);
            for off in [
                FRAMES,
                CLOSED,
                POINTER_X,
                POINTER_Y,
                POINTER_DOWN,
                RESIZED,
                TIME,
            ] {
                self.vk_imm(off, 0);
            }
            self.vk_imm(KEY, u64::MAX);
            self.vk_imm(HEADLESS, u64::from(matches!(op, VulkanOp::OpenHeadless)));
            self.branch(self.vulkan.setup, Branch::Call);
            self.branch(failed, Branch::Zero(0));
            if matches!(op, VulkanOp::Open) {
                self.load(1);
                self.push(0);
                self.load(2);
                self.push(0);
                self.load(3);
                self.mov(2, 0);
                self.pop(1);
                self.pop(0);
                self.branch(self.vulkan.window_open, Branch::Call);
                self.branch(failed, Branch::Zero(0));
            }
            self.vk_get(GEN, 0);
            self.add_imm(0, 0, 1);
            self.vk_set(GEN, 0);
            self.vk_field(ty, "window.id", 0);
            self.vk_clock();
            self.vk_set(368, 0);
            self.vk_imm(FIRST_POLL, 1);
            self.vk_imm(ACTIVE, 1);
            self.vk_ok(ty);
            self.mark(failed);
            self.branch(self.vulkan.destroy, Branch::Call);
            self.vk_error(ty, "Vulkan or SDL initialization failed");
            self.mark(occupied);
            self.vk_error(ty, "a Vulkan renderer is already open");
            self.mark(dimensions);
            self.vk_error(ty, "Vulkan dimensions must be between 1 and 4096");
            self.mark(unavailable);
            self.vk_error(ty, "Vulkan loader or required entry points are unavailable");
        }
        let ty = self.vk_begin(VulkanOp::Poll);
        let invalid = self.label();
        let ready = self.label();
        let headless = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.vulkan.validate, Branch::Call);
        self.branch(invalid, Branch::Zero(0));
        self.vk_get(HEADLESS, 0);
        self.branch(headless, Branch::Nonzero(0));
        self.branch(self.vulkan.window_poll, Branch::Call);
        self.branch(ready, Branch::Jump);
        self.mark(headless);
        self.vk_clock();
        self.vk_get(368, 1);
        self.sub(0, 0, 1);
        self.vk_set(TIME, 0);
        self.mark(ready);
        let not_first = self.label();
        self.vk_get(FIRST_POLL, 0);
        self.branch(not_first, Branch::Zero(0));
        self.vk_imm(RESIZED, 1);
        self.vk_imm(FIRST_POLL, 0);
        self.mark(not_first);
        for (name, off) in [
            ("closed", CLOSED),
            ("width", WINDOW_WIDTH),
            ("height", WINDOW_HEIGHT),
            ("pixel_width", WIDTH),
            ("pixel_height", HEIGHT),
            ("pointer_down", POINTER_DOWN),
            ("key", KEY),
            ("resized", RESIZED),
        ] {
            self.vk_get(off, 0);
            self.vk_field(ty, name, 0);
        }
        self.imm(0, 1.0f64.to_bits());
        self.vk_field(ty, "scale", 0);
        for (name, off) in [
            ("pointer_x", POINTER_X),
            ("pointer_y", POINTER_Y),
            ("time", TIME),
        ] {
            self.vk_get(off, 0);
            self.emit(0x9e620000);
            if name == "time" {
                self.imm(1, 1000.0f64.to_bits());
                self.bits_to_fp(1, 1, Type::F64);
                self.emit(0x1e611800);
            }
            self.fp_to_bits(0, 0, Type::F64);
            self.vk_field(ty, name, 0);
        }
        self.vk_imm(RESIZED, 0);
        self.vk_return();
        self.mark(invalid);
        self.imm(0, 1);
        self.vk_field(ty, "closed", 0);
        self.imm(0, u64::MAX);
        self.vk_field(ty, "key", 0);
        self.vk_return();
        let ty = self.vk_begin(VulkanOp::Draw);
        let invalid = self.label();
        let geometry = self.label();
        let failed = self.label();
        let closed = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.vulkan.validate, Branch::Call);
        self.branch(invalid, Branch::Zero(0));
        self.vk_get(FRAMES, 0);
        self.vk_field(ty, "frames", 0);
        self.vk_get(CLOSED, 0);
        self.branch(closed, Branch::Nonzero(0));
        self.load(2);
        self.add_imm(0, 0, 8);
        self.ldr(0, 0);
        self.store(6, 0);
        self.imm(1, 800000);
        self.cmp(0, 1);
        self.branch(geometry, Branch::Cond(8));
        self.imm(1, 24);
        self.emit(0x9ac10802);
        self.emit(0x9b018040);
        self.branch(geometry, Branch::Nonzero(0));
        // Validate every uploaded float; malformed values never enter the GPU driver.
        self.load(2);
        self.ldr(10, 0);
        self.load(6);
        self.mov(11, 0);
        let values = self.label();
        let values_done = self.label();
        self.mark(values);
        self.branch(values_done, Branch::Zero(11));
        self.emit(0xb940014c);
        self.imm(13, 0x7f800000);
        self.emit(0x8a0d018c);
        self.cmp(12, 13);
        self.branch(geometry, Branch::Cond(0));
        self.add_imm(10, 10, 4);
        self.sub_imm(11, 11, 1);
        self.branch(values, Branch::Jump);
        self.mark(values_done);
        for (slot, off) in [(3, 336), (4, 344), (5, 352)] {
            self.load(slot);
            self.imm(1, 0x7ff0000000000000);
            self.emit(0x8a010002);
            self.cmp(2, 1);
            self.branch(geometry, Branch::Cond(0));
            self.bits_to_fp(0, 0, Type::F64);
            self.emit(0x1e624000);
            self.fp_to_bits(0, 0, Type::F32);
            self.vk_set(off, 0);
        }
        self.load(2);
        self.ldr(10, 0);
        self.vk_get(VERTEX_MAP, 11);
        self.load(6);
        self.lsl(12, 0, 2);
        self.copy_bytes(11, 10, 12, 13);
        self.load(6);
        self.lsr(0, 0, 3);
        self.vk_set(360, 0);
        self.branch(self.vulkan.render, Branch::Call);
        self.branch(failed, Branch::Zero(0));
        let no_window = self.label();
        self.vk_get(HEADLESS, 0);
        self.branch(no_window, Branch::Nonzero(0));
        self.branch(self.vulkan.window_present, Branch::Call);
        self.branch(failed, Branch::Zero(0));
        self.mark(no_window);
        self.vk_get(FRAMES, 0);
        self.add_imm(0, 0, 1);
        self.vk_set(FRAMES, 0);
        self.vk_field(ty, "frames", 0);
        self.vk_ok(ty);
        self.mark(invalid);
        self.vk_error(ty, "invalid or expired Vulkan handle");
        self.mark(closed);
        self.vk_error(ty, "Vulkan window is closed");
        self.mark(geometry);
        self.vk_error(ty,"vertices must contain finite triangle-list floats (max 100000 vertices), with finite clear colors");
        self.mark(failed);
        self.vk_error(ty, "Vulkan rendering or window presentation failed");
        self.vk_begin(VulkanOp::Close);
        let done = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.vulkan.validate, Branch::Call);
        self.branch(done, Branch::Zero(0));
        self.branch(self.vulkan.destroy, Branch::Call);
        self.mark(done);
        self.imm(0, 0);
        self.epilogue();
        self.vk_begin(VulkanOp::DeviceName);
        let empty = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.vulkan.validate, Branch::Call);
        self.branch(empty, Branch::Zero(0));
        self.vk_get(NAME, 0);
        self.branch(self.strings.alloc, Branch::Call);
        self.store(6, 0);
        self.add_imm(11, 0, 8);
        self.vk_addr(NAME + 8, 10);
        self.vk_get(NAME, 12);
        self.copy_bytes(11, 10, 12, 13);
        self.load(6);
        self.epilogue();
        self.mark(empty);
        let emptystr = self.string_literal("");
        self.branch(emptystr, Branch::Address(0));
        self.epilogue();
        self.vk_begin(VulkanOp::Pixels);
        self.load(0);
        self.str(31, 0);
        self.add_imm(9, 0, 8);
        self.str(31, 9);
        self.add_imm(9, 0, 16);
        self.str(31, 9);
        let empty_pixels = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.vulkan.validate, Branch::Call);
        self.branch(empty_pixels, Branch::Zero(0));
        self.vk_get(FRAMES, 0);
        self.branch(empty_pixels, Branch::Zero(0));
        self.vk_get(PIXEL_BYTES, 1);
        self.load(0);
        self.imm(2, 4);
        self.branch(self.collections.grow, Branch::Call);
        self.vk_get(PIXEL_BYTES, 12);
        self.add_imm(9, 0, 8);
        self.str(12, 9);
        self.ldr(11, 0);
        self.vk_get(PIXELS, 10);
        let pixels_loop = self.label();
        let pixels_done = self.label();
        self.mark(pixels_loop);
        self.branch(pixels_done, Branch::Zero(12));
        self.ldrb(13, 10);
        self.typed_store_reg(Type::I32, 11, 13);
        self.add_imm(10, 10, 1);
        self.add_imm(11, 11, 4);
        self.sub_imm(12, 12, 1);
        self.branch(pixels_loop, Branch::Jump);
        self.mark(pixels_done);
        self.mark(empty_pixels);
        self.vk_return();
    }
}
