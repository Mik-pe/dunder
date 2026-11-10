use super::*;
const CLEAR_R: u32 = 336;
const CLEAR_G: u32 = 344;
const CLEAR_B: u32 = 352;
const VERTICES: u32 = 360;
impl Code<'_> {
    fn vk_fields(&mut self, base: u32, fields: &[(u32, u32)]) {
        for (off, n) in fields {
            self.vk_u32(base + off, *n);
        }
    }
    pub(super) fn vk_setup_runtime(&mut self) {
        self.mark(self.vulkan.setup);
        self.prologue(FRAME);
        let failed = self.label();
        let s = SCRATCH;
        self.vk_zero(s, 4096);
        // VkApplicationInfo and VkInstanceCreateInfo: no optional extensions/layers.
        self.vk_u32(s + 44, 1 << 22);
        self.vk_pointer(s + 64 + 24, s);
        self.vk_u32(s + 64, 1);
        self.vk_call(
            "vkCreateInstance",
            &[Arg::Addr(s + 64), Arg::Imm(0), Arg::Addr(INSTANCE)],
        );
        self.vk_result(failed);
        self.vk_u32(s, 16);
        self.vk_call(
            "vkEnumeratePhysicalDevices",
            &[Arg::Ctx(INSTANCE), Arg::Addr(s), Arg::Addr(s + 64)],
        );
        self.vk_result(failed);
        self.vk_read32(s, 0);
        self.branch(failed, Branch::Zero(0));
        self.vk_get(s + 64, 0);
        self.vk_set(PHYSICAL, 0);
        self.vk_call(
            "vkGetPhysicalDeviceProperties",
            &[Arg::Ctx(PHYSICAL), Arg::Addr(s)],
        );
        self.vk_addr(s + 20, 10);
        self.vk_addr(NAME + 8, 11);
        self.imm(12, 0);
        let name_loop = self.label();
        let name_done = self.label();
        self.mark(name_loop);
        self.ldrb(13, 10);
        self.branch(name_done, Branch::Zero(13));
        self.strb(13, 11);
        self.add_imm(10, 10, 1);
        self.add_imm(11, 11, 1);
        self.add_imm(12, 12, 1);
        self.imm(14, 255);
        self.cmp(12, 14);
        self.branch(name_loop, Branch::Cond(3));
        self.mark(name_done);
        self.vk_set(NAME, 12);
        self.vk_call(
            "vkGetPhysicalDeviceMemoryProperties",
            &[Arg::Ctx(PHYSICAL), Arg::Addr(MEMORY_PROPERTIES)],
        );
        self.vk_u32(s, 32);
        self.vk_call(
            "vkGetPhysicalDeviceQueueFamilyProperties",
            &[Arg::Ctx(PHYSICAL), Arg::Addr(s), Arg::Addr(s + 64)],
        );
        self.vk_read32(s, 11);
        self.imm(12, 0);
        self.vk_addr(s + 64, 10);
        let family_loop = self.label();
        let family_found = self.label();
        self.mark(family_loop);
        self.cmp(12, 11);
        self.branch(failed, Branch::Cond(2));
        self.emit(0xb940014d);
        self.imm(14, 1);
        self.emit(0x8a0e01ad);
        self.branch(family_found, Branch::Nonzero(13));
        self.add_imm(10, 10, 24);
        self.add_imm(12, 12, 1);
        self.branch(family_loop, Branch::Jump);
        self.mark(family_found);
        self.vk_set(FAMILY, 12);
        self.vk_zero(s, 512);
        self.vk_u32(s, 2);
        self.vk_get(FAMILY, 0);
        self.vk_write32(s + 20, 0);
        self.vk_u32(s + 24, 1);
        self.vk_pointer(s + 32, s + 160);
        self.vk_u32(s + 160, 1.0f32.to_bits());
        self.vk_u32(s + 64, 3);
        self.vk_u32(s + 64 + 20, 1);
        self.vk_pointer(s + 64 + 24, s);
        self.vk_call(
            "vkCreateDevice",
            &[
                Arg::Ctx(PHYSICAL),
                Arg::Addr(s + 64),
                Arg::Imm(0),
                Arg::Addr(DEVICE),
            ],
        );
        self.vk_result(failed);
        self.vk_call(
            "vkGetDeviceQueue",
            &[
                Arg::Ctx(DEVICE),
                Arg::Ctx(FAMILY),
                Arg::Imm(0),
                Arg::Addr(QUEUE),
            ],
        );
        self.vk_zero(s, 64);
        self.vk_u32(s, 39);
        self.vk_u32(s + 16, 2);
        self.vk_get(FAMILY, 0);
        self.vk_write32(s + 20, 0);
        self.vk_call(
            "vkCreateCommandPool",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(COMMAND_POOL),
            ],
        );
        self.vk_result(failed);
        self.vk_zero(s, 64);
        self.vk_u32(s, 40);
        self.vk_get(COMMAND_POOL, 0);
        self.vk_set(s + 16, 0);
        self.vk_u32(s + 28, 1);
        self.vk_call(
            "vkAllocateCommandBuffers",
            &[Arg::Ctx(DEVICE), Arg::Addr(s), Arg::Addr(COMMAND)],
        );
        self.vk_result(failed);
        // Optimal tiling RGBA8 color attachment, copied into coherent host memory.
        self.vk_zero(s, 128);
        self.vk_fields(
            s,
            &[
                (0, 14),
                (20, 1),
                (24, 37),
                (36, 1),
                (40, 1),
                (44, 1),
                (48, 1),
                (56, 0x11),
            ],
        );
        self.vk_get(WIDTH, 0);
        self.vk_write32(s + 28, 0);
        self.vk_get(HEIGHT, 0);
        self.vk_write32(s + 32, 0);
        self.vk_call(
            "vkCreateImage",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(IMAGE),
            ],
        );
        self.vk_result(failed);
        self.vk_call(
            "vkGetImageMemoryRequirements",
            &[Arg::Ctx(DEVICE), Arg::Ctx(IMAGE), Arg::Addr(s + 128)],
        );
        self.vk_read32(s + 144, 0);
        self.imm(1, 0);
        self.branch(self.vulkan.memory_type, Branch::Call);
        self.imm(1, u64::MAX);
        self.cmp(0, 1);
        self.branch(failed, Branch::Cond(0));
        self.store(1, 0);
        self.vk_zero(s, 64);
        self.vk_u32(s, 5);
        self.vk_get(s + 128, 0);
        self.vk_set(s + 16, 0);
        self.load(1);
        self.vk_write32(s + 24, 0);
        self.vk_call(
            "vkAllocateMemory",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(IMAGE_MEMORY),
            ],
        );
        self.vk_result(failed);
        self.vk_call(
            "vkBindImageMemory",
            &[
                Arg::Ctx(DEVICE),
                Arg::Ctx(IMAGE),
                Arg::Ctx(IMAGE_MEMORY),
                Arg::Imm(0),
            ],
        );
        self.vk_result(failed);
        self.vk_zero(s, 96);
        self.vk_fields(s, &[(0, 15), (32, 1), (36, 37), (56, 1), (64, 1), (72, 1)]);
        self.vk_get(IMAGE, 0);
        self.vk_set(s + 24, 0);
        self.vk_call(
            "vkCreateImageView",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(IMAGE_VIEW),
            ],
        );
        self.vk_result(failed);
        // Attachment, subpass, and explicit transfer dependency.
        self.vk_zero(s, 512);
        self.vk_fields(
            s + 128,
            &[(4, 37), (8, 1), (12, 1), (20, 2), (24, 1), (32, 6)],
        );
        self.vk_u32(s + 176 + 4, 2);
        self.vk_u32(s + 192 + 24, 1);
        self.vk_pointer(s + 192 + 32, s + 176);
        self.vk_fields(s + 272, &[(0, u32::MAX), (8, 1), (12, 0x400), (20, 0x100)]);
        self.vk_fields(
            s + 300,
            &[
                (4, u32::MAX),
                (8, 0x400),
                (12, 0x1000),
                (16, 0x100),
                (20, 0x800),
            ],
        );
        self.vk_fields(s, &[(0, 38), (20, 1), (32, 1), (48, 2)]);
        self.vk_pointer(s + 24, s + 128);
        self.vk_pointer(s + 40, s + 192);
        self.vk_pointer(s + 56, s + 272);
        self.vk_call(
            "vkCreateRenderPass",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(RENDER_PASS),
            ],
        );
        self.vk_result(failed);
        self.vk_zero(s, 96);
        self.vk_fields(s, &[(0, 37), (32, 1), (56, 1)]);
        self.vk_get(RENDER_PASS, 0);
        self.vk_set(s + 24, 0);
        self.vk_pointer(s + 40, IMAGE_VIEW);
        self.vk_get(WIDTH, 0);
        self.vk_write32(s + 48, 0);
        self.vk_get(HEIGHT, 0);
        self.vk_write32(s + 52, 0);
        self.vk_call(
            "vkCreateFramebuffer",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(FRAMEBUFFER),
            ],
        );
        self.vk_result(failed);
        for (words, destination) in [
            (shaders::VERTEX_SPIRV, VERTEX_SHADER),
            (shaders::FRAGMENT_SPIRV, FRAGMENT_SHADER),
        ] {
            self.vk_zero(s, 64);
            self.vk_u32(s, 16);
            self.vk_imm(s + 24, (words.len() * 4) as u64);
            self.vk_pointer(s + 32, s + 512);
            for (i, word) in words.iter().enumerate() {
                self.vk_u32(s + 512 + i as u32 * 4, *word);
            }
            self.vk_call(
                "vkCreateShaderModule",
                &[
                    Arg::Ctx(DEVICE),
                    Arg::Addr(s),
                    Arg::Imm(0),
                    Arg::Addr(destination),
                ],
            );
            self.vk_result(failed);
        }
        self.vk_zero(s, 64);
        self.vk_u32(s, 30);
        self.vk_call(
            "vkCreatePipelineLayout",
            &[
                Arg::Ctx(DEVICE),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(PIPELINE_LAYOUT),
            ],
        );
        self.vk_result(failed);
        self.vk_pipeline(failed);
        // Helper creates and maps coherent buffers (size,usage,destination-offset).
        self.imm(0, VERTEX_BYTES);
        self.imm(1, 0x80);
        self.imm(2, VERTEX_BUFFER as u64);
        self.branch(self.vulkan.buffer, Branch::Call);
        self.branch(failed, Branch::Zero(0));
        self.vk_get(WIDTH, 0);
        self.vk_get(HEIGHT, 1);
        self.emit(0x9b017c00);
        self.lsl(0, 0, 2);
        self.vk_set(PIXEL_BYTES, 0);
        self.imm(1, 2);
        self.imm(2, READ_BUFFER as u64);
        self.branch(self.vulkan.buffer, Branch::Call);
        self.branch(failed, Branch::Zero(0));
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
        self.vk_memory_runtime();
        self.vk_buffer_runtime();
    }
    fn vk_pipeline(&mut self, failed: Label) {
        let s = SCRATCH;
        self.vk_zero(s, 1536);
        for (off, stage, module) in [(256, 1, VERTEX_SHADER), (304, 16, FRAGMENT_SHADER)] {
            self.vk_fields(s + off, &[(0, 18), (20, stage)]);
            self.vk_get(module, 0);
            self.vk_set(s + off + 24, 0);
            self.vk_cstr("main", 0);
            self.vk_set(s + off + 32, 0);
        }
        self.vk_fields(s + 352, &[(0, 19), (20, 1), (32, 2)]);
        self.vk_pointer(s + 352 + 24, s + 400);
        self.vk_pointer(s + 352 + 40, s + 416);
        self.vk_u32(s + 400 + 4, 32);
        self.vk_fields(s + 416, &[(8, 109)]);
        self.vk_fields(s + 432, &[(0, 1), (8, 109), (12, 16)]);
        self.vk_fields(s + 448, &[(0, 20), (20, 3)]);
        self.vk_fields(s + 480, &[(0, 22), (20, 1), (32, 1)]);
        self.vk_pointer(s + 480 + 24, s + 544);
        self.vk_pointer(s + 480 + 40, s + 576);
        for (source, off) in [(WIDTH, 8), (HEIGHT, 12)] {
            self.vk_get(source, 0);
            self.emit(0x9e220000);
            self.fp_to_bits(0, 0, Type::F32);
            self.vk_write32(s + 544 + off, 0);
        }
        self.vk_u32(s + 544 + 20, 1.0f32.to_bits());
        self.vk_get(WIDTH, 0);
        self.vk_write32(s + 576 + 8, 0);
        self.vk_get(HEIGHT, 0);
        self.vk_write32(s + 576 + 12, 0);
        self.vk_u32(s + 608, 23);
        self.vk_u32(s + 608 + 56, 1.0f32.to_bits());
        self.vk_fields(s + 672, &[(0, 24), (20, 1)]);
        self.vk_fields(s + 736, &[(0, 26), (28, 1)]);
        self.vk_pointer(s + 736 + 32, s + 800);
        self.vk_fields(
            s + 800,
            &[(0, 1), (4, 6), (8, 7), (16, 1), (20, 7), (28, 15)],
        );
        self.vk_fields(s, &[(0, 28), (20, 2), (136, u32::MAX)]);
        for (off, target) in [
            (24, 256),
            (32, 352),
            (40, 448),
            (56, 480),
            (64, 608),
            (72, 672),
            (88, 736),
        ] {
            self.vk_pointer(s + off, s + target);
        }
        self.vk_get(PIPELINE_LAYOUT, 0);
        self.vk_set(s + 104, 0);
        self.vk_get(RENDER_PASS, 0);
        self.vk_set(s + 112, 0);
        self.vk_call(
            "vkCreateGraphicsPipelines",
            &[
                Arg::Ctx(DEVICE),
                Arg::Imm(0),
                Arg::Imm(1),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Addr(PIPELINE),
            ],
        );
        self.vk_result(failed);
    }
    fn vk_memory_runtime(&mut self) {
        self.mark(self.vulkan.memory_type);
        self.mov(12, 0);
        self.mov(13, 1);
        self.vk_read32(MEMORY_PROPERTIES, 11);
        self.vk_addr(MEMORY_PROPERTIES + 4, 10);
        self.imm(0, 0);
        self.imm(14, 1);
        let head = self.label();
        let next = self.label();
        let done = self.label();
        self.mark(head);
        self.cmp(0, 11);
        self.branch(done, Branch::Cond(2));
        self.emit(0x8a0e018f);
        self.branch(next, Branch::Zero(15));
        self.emit(0xb940014f);
        self.emit(0x8a0d01ef);
        self.cmp(15, 13);
        self.ret_cond(0);
        self.mark(next);
        self.add_imm(0, 0, 1);
        self.lsl(14, 14, 1);
        self.add_imm(10, 10, 8);
        self.branch(head, Branch::Jump);
        self.mark(done);
        self.imm(0, u64::MAX);
        self.ret();
    }
    fn ret_cond(&mut self, cond: u32) {
        let skip = self.label();
        self.branch(skip, Branch::Cond(cond ^ 1));
        self.ret();
        self.mark(skip);
    }
    fn vk_buffer_runtime(&mut self) {
        self.mark(self.vulkan.buffer);
        self.prologue(FRAME);
        self.store(1, 0);
        self.store(2, 1);
        self.store(3, 2);
        let failed = self.label();
        let s = SCRATCH;
        self.vk_zero(s, 256);
        self.vk_u32(s, 12);
        self.load(1);
        self.vk_set(s + 24, 0);
        self.load(2);
        self.vk_write32(s + 32, 0);
        self.load(3);
        self.vk_ctx(2);
        self.add(2, 2, 0);
        self.store(4, 2);
        self.vk_call(
            "vkCreateBuffer",
            &[Arg::Ctx(DEVICE), Arg::Addr(s), Arg::Imm(0), Arg::Slot(4)],
        );
        self.vk_result(failed);
        self.load(4);
        self.ldr(0, 0);
        self.store(5, 0);
        self.vk_call(
            "vkGetBufferMemoryRequirements",
            &[Arg::Ctx(DEVICE), Arg::Slot(5), Arg::Addr(s + 128)],
        );
        self.vk_read32(s + 144, 0);
        self.imm(1, 6);
        self.branch(self.vulkan.memory_type, Branch::Call);
        self.imm(1, u64::MAX);
        self.cmp(0, 1);
        self.branch(failed, Branch::Cond(0));
        self.store(6, 0);
        self.vk_zero(s, 64);
        self.vk_u32(s, 5);
        self.vk_get(s + 128, 0);
        self.vk_set(s + 16, 0);
        self.load(6);
        self.vk_write32(s + 24, 0);
        self.load(4);
        self.add_imm(0, 0, 8);
        self.store(7, 0);
        self.vk_call(
            "vkAllocateMemory",
            &[Arg::Ctx(DEVICE), Arg::Addr(s), Arg::Imm(0), Arg::Slot(7)],
        );
        self.vk_result(failed);
        self.load(7);
        self.ldr(0, 0);
        self.store(8, 0);
        self.vk_call(
            "vkBindBufferMemory",
            &[Arg::Ctx(DEVICE), Arg::Slot(5), Arg::Slot(8), Arg::Imm(0)],
        );
        self.vk_result(failed);
        self.load(4);
        self.add_imm(0, 0, 16);
        self.store(9, 0);
        self.vk_call(
            "vkMapMemory",
            &[
                Arg::Ctx(DEVICE),
                Arg::Slot(8),
                Arg::Imm(0),
                Arg::Imm(u64::MAX),
                Arg::Imm(0),
                Arg::Slot(9),
            ],
        );
        self.vk_result(failed);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
    }
    pub(super) fn vk_render_runtime(&mut self) {
        self.mark(self.vulkan.render);
        self.prologue(FRAME);
        let failed = self.label();
        let s = SCRATCH;
        self.vk_call("vkResetCommandBuffer", &[Arg::Ctx(COMMAND), Arg::Imm(0)]);
        self.vk_result(failed);
        self.vk_zero(s, 96);
        self.vk_u32(s, 42);
        self.vk_u32(s + 16, 1);
        self.vk_call("vkBeginCommandBuffer", &[Arg::Ctx(COMMAND), Arg::Addr(s)]);
        self.vk_result(failed);
        self.vk_zero(s, 256);
        self.vk_u32(s, 43);
        self.vk_get(RENDER_PASS, 0);
        self.vk_set(s + 16, 0);
        self.vk_get(FRAMEBUFFER, 0);
        self.vk_set(s + 24, 0);
        self.vk_get(WIDTH, 0);
        self.vk_write32(s + 40, 0);
        self.vk_get(HEIGHT, 0);
        self.vk_write32(s + 44, 0);
        self.vk_u32(s + 48, 1);
        self.vk_pointer(s + 56, s + 128);
        for (off, source) in [(0, CLEAR_R), (4, CLEAR_G), (8, CLEAR_B)] {
            self.vk_get(source, 0);
            self.vk_write32(s + 128 + off, 0);
        }
        self.vk_u32(s + 140, 1.0f32.to_bits());
        self.vk_call(
            "vkCmdBeginRenderPass",
            &[Arg::Ctx(COMMAND), Arg::Addr(s), Arg::Imm(0)],
        );
        self.vk_call(
            "vkCmdBindPipeline",
            &[Arg::Ctx(COMMAND), Arg::Imm(0), Arg::Ctx(PIPELINE)],
        );
        self.vk_imm(s + 192, 0);
        self.vk_call(
            "vkCmdBindVertexBuffers",
            &[
                Arg::Ctx(COMMAND),
                Arg::Imm(0),
                Arg::Imm(1),
                Arg::Addr(VERTEX_BUFFER),
                Arg::Addr(s + 192),
            ],
        );
        self.vk_call(
            "vkCmdDraw",
            &[
                Arg::Ctx(COMMAND),
                Arg::Ctx(VERTICES),
                Arg::Imm(1),
                Arg::Imm(0),
                Arg::Imm(0),
            ],
        );
        self.vk_call("vkCmdEndRenderPass", &[Arg::Ctx(COMMAND)]);
        self.vk_zero(s, 64);
        self.vk_fields(s, &[(16, 1), (28, 1), (52, 1)]);
        self.vk_get(WIDTH, 0);
        self.vk_write32(s + 44, 0);
        self.vk_get(HEIGHT, 0);
        self.vk_write32(s + 48, 0);
        self.vk_call(
            "vkCmdCopyImageToBuffer",
            &[
                Arg::Ctx(COMMAND),
                Arg::Ctx(IMAGE),
                Arg::Imm(6),
                Arg::Ctx(READ_BUFFER),
                Arg::Imm(1),
                Arg::Addr(s),
            ],
        );
        self.vk_zero(s, 32);
        self.vk_fields(s, &[(0, 46), (16, 0x1000), (20, 0x2000)]);
        self.vk_call(
            "vkCmdPipelineBarrier",
            &[
                Arg::Ctx(COMMAND),
                Arg::Imm(0x1000),
                Arg::Imm(0x4000),
                Arg::Imm(0),
                Arg::Imm(1),
                Arg::Addr(s),
                Arg::Imm(0),
                Arg::Imm(0),
                Arg::Imm(0),
                Arg::Imm(0),
            ],
        );
        self.vk_call("vkEndCommandBuffer", &[Arg::Ctx(COMMAND)]);
        self.vk_result(failed);
        self.vk_zero(s, 80);
        self.vk_fields(s, &[(0, 4), (40, 1)]);
        self.vk_pointer(s + 48, COMMAND);
        self.vk_call(
            "vkQueueSubmit",
            &[Arg::Ctx(QUEUE), Arg::Imm(1), Arg::Addr(s), Arg::Imm(0)],
        );
        self.vk_result(failed);
        self.vk_call("vkQueueWaitIdle", &[Arg::Ctx(QUEUE)]);
        self.vk_result(failed);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
    }
    pub(super) fn vk_destroy_runtime(&mut self) {
        self.mark(self.vulkan.destroy);
        self.prologue(FRAME);
        let no_device = self.label();
        self.vk_get(DEVICE, 0);
        self.branch(no_device, Branch::Zero(0));
        self.vk_call("vkDeviceWaitIdle", &[Arg::Ctx(DEVICE)]);
        for (map, memory) in [(VERTEX_MAP, VERTEX_MEMORY), (PIXELS, READ_MEMORY)] {
            let skip = self.label();
            self.vk_get(map, 0);
            self.branch(skip, Branch::Zero(0));
            self.vk_call("vkUnmapMemory", &[Arg::Ctx(DEVICE), Arg::Ctx(memory)]);
            self.mark(skip);
            self.vk_imm(map, 0);
        }
        for (off, name) in [
            (VERTEX_BUFFER, "vkDestroyBuffer"),
            (READ_BUFFER, "vkDestroyBuffer"),
            (FRAMEBUFFER, "vkDestroyFramebuffer"),
            (PIPELINE, "vkDestroyPipeline"),
            (PIPELINE_LAYOUT, "vkDestroyPipelineLayout"),
            (VERTEX_SHADER, "vkDestroyShaderModule"),
            (FRAGMENT_SHADER, "vkDestroyShaderModule"),
            (RENDER_PASS, "vkDestroyRenderPass"),
            (IMAGE_VIEW, "vkDestroyImageView"),
            (IMAGE, "vkDestroyImage"),
            (VERTEX_MEMORY, "vkFreeMemory"),
            (READ_MEMORY, "vkFreeMemory"),
            (IMAGE_MEMORY, "vkFreeMemory"),
            (COMMAND_POOL, "vkDestroyCommandPool"),
        ] {
            let skip = self.label();
            self.vk_get(off, 0);
            self.branch(skip, Branch::Zero(0));
            self.vk_call(name, &[Arg::Ctx(DEVICE), Arg::Ctx(off), Arg::Imm(0)]);
            self.mark(skip);
            self.vk_imm(off, 0);
        }
        self.vk_call("vkDestroyDevice", &[Arg::Ctx(DEVICE), Arg::Imm(0)]);
        self.vk_imm(DEVICE, 0);
        self.mark(no_device);
        let no_instance = self.label();
        self.vk_get(INSTANCE, 0);
        self.branch(no_instance, Branch::Zero(0));
        self.vk_call("vkDestroyInstance", &[Arg::Ctx(INSTANCE), Arg::Imm(0)]);
        self.vk_imm(INSTANCE, 0);
        self.mark(no_instance);
        self.branch(self.vulkan.window_close, Branch::Call);
        self.vk_imm(ACTIVE, 0);
        self.vk_imm(CLOSED, 1);
        self.epilogue();
    }
}
