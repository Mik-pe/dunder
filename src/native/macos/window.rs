//! AppKit ownership, bounded event polling, Retina geometry and native callbacks.
//! Callbacks intentionally never enter the Dunder ABI or its allocation arenas.
use super::*;

impl Code<'_> {
    pub(super) fn mac_window_runtime(&mut self) {
        self.mac_window_open();
        self.mac_window_poll();
        self.mac_window_close();
        self.mac_window_resize();
        self.mac_window_destroy();
        self.mac_window_callbacks();
    }

    fn mac_window_open(&mut self) {
        let ty = self.mac_begin(MacOp::Open);
        let unavailable = self.label();
        let occupied = self.label();
        let dimensions = self.label();
        let failed = self.label();
        for slot in [2, 3] {
            self.load(slot);
            self.imm(1, 1);
            self.cmp(0, 1);
            self.branch(dimensions, Branch::Cond(11));
            self.imm(1, 16384);
            self.cmp(0, 1);
            self.branch(dimensions, Branch::Cond(12));
        }
        self.branch(self.macos.init, Branch::Call);
        self.branch(unavailable, Branch::Zero(0));
        self.mac_get(ACTIVE, 0);
        self.branch(occupied, Branch::Nonzero(0));
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.store(6, 0);
        self.branch(self.macos.setup, Branch::Call);
        self.branch(failed, Branch::Zero(0));

        // Locals 10/11 hold doubles; CGRect is a four-double HFA.
        for (src, dst) in [(2, 10), (3, 11)] {
            self.load(src);
            self.emit(0x9e620000); // scvtf d0,x0
            self.mac_save_fp(dst, 0);
        }
        self.mac_msg(Arg::Ctx(CL_WINDOW), "alloc", &[], &[]);
        self.store(12, 0);
        self.mac_msg(
            Arg::Slot(12),
            "initWithContentRect:styleMask:backing:defer:",
            &[Arg::Imm(15), Arg::Imm(2), Arg::Imm(0)],
            &[
                FArg::Value(0.0),
                FArg::Value(0.0),
                FArg::Slot(10),
                FArg::Slot(11),
            ],
        );
        self.mac_set(WINDOW, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(
            Arg::Ctx(WINDOW),
            "setReleasedWhenClosed:",
            &[Arg::Imm(0)],
            &[],
        );

        self.mac_msg(Arg::Ctx(CL_STRING), "alloc", &[], &[]);
        self.store(12, 0);
        self.load(1);
        self.ldr(1, 0);
        self.store(14, 1);
        self.add_imm(0, 0, 8);
        self.store(13, 0);
        self.mac_msg(
            Arg::Slot(12),
            "initWithBytes:length:encoding:",
            &[Arg::Slot(13), Arg::Slot(14), Arg::Imm(4)],
            &[],
        );
        self.store(12, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "setTitle:", &[Arg::Slot(12)], &[]);
        self.mac_msg(Arg::Slot(12), "release", &[], &[]);
        self.mac_msg(
            Arg::Ctx(WINDOW),
            "setContentMinSize:",
            &[],
            &[FArg::Value(64.0), FArg::Value(64.0)],
        );
        self.mac_msg(
            Arg::Ctx(WINDOW),
            "setAcceptsMouseMovedEvents:",
            &[Arg::Imm(1)],
            &[],
        );

        self.mac_msg(Arg::Ctx(CL_VIEW), "alloc", &[], &[]);
        self.store(12, 0);
        self.mac_msg(
            Arg::Slot(12),
            "initWithFrame:",
            &[],
            &[
                FArg::Value(0.0),
                FArg::Value(0.0),
                FArg::Slot(10),
                FArg::Slot(11),
            ],
        );
        self.mac_set(VIEW, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(VIEW), "setAutoresizingMask:", &[Arg::Imm(18)], &[]);
        self.mac_msg(Arg::Ctx(CL_LAYER), "new", &[], &[]);
        self.mac_set(LAYER, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(LAYER), "setDevice:", &[Arg::Ctx(DEVICE)], &[]);
        self.mac_msg(Arg::Ctx(LAYER), "setPixelFormat:", &[Arg::Imm(80)], &[]);
        self.mac_msg(Arg::Ctx(LAYER), "setFramebufferOnly:", &[Arg::Imm(1)], &[]);
        self.mac_msg(Arg::Ctx(VIEW), "setLayer:", &[Arg::Ctx(LAYER)], &[]);
        self.mac_msg(Arg::Ctx(VIEW), "setWantsLayer:", &[Arg::Imm(1)], &[]);
        self.mac_msg(Arg::Ctx(WINDOW), "setContentView:", &[Arg::Ctx(VIEW)], &[]);

        self.mac_msg(Arg::Ctx(DELEGATE_CLASS), "new", &[], &[]);
        self.mac_set(DELEGATE, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "setDelegate:", &[Arg::Ctx(DELEGATE)], &[]);
        self.mac_msg(Arg::Ctx(APP), "setDelegate:", &[Arg::Ctx(DELEGATE)], &[]);
        self.mac_imm(CLOSED, 0);
        self.mac_imm(POINTER_DOWN, 0);
        self.mac_imm(POINTER_X, 0);
        self.mac_imm(POINTER_Y, 0);
        self.mac_imm(KEY, u64::MAX);
        self.mac_imm(RESIZED, 1);
        self.mac_imm(FRAMES, 0);
        self.mac_imm(ERROR_CODE, 0);
        self.mac_get(GEN, 0);
        self.add_imm(0, 0, 1);
        self.mac_set(GEN, 0);
        self.mac_imm(ACTIVE, 1);
        self.mac_msg(Arg::Ctx(WINDOW), "center", &[], &[]);
        self.mac_msg(
            Arg::Ctx(WINDOW),
            "makeKeyAndOrderFront:",
            &[Arg::Imm(0)],
            &[],
        );
        self.mac_msg(
            Arg::Ctx(APP),
            "activateIgnoringOtherApps:",
            &[Arg::Imm(1)],
            &[],
        );
        self.branch(self.macos.resize, Branch::Call);
        self.mac_get(FN_MEDIA_TIME, 16);
        self.emit(0xd63f0200); // blr x16; CACurrentMediaTime returns d0
        self.mac_store_fp(START_TIME, 0);
        self.mac_msg(Arg::Slot(6), "drain", &[], &[]);
        self.mac_get(GEN, 0);
        self.mac_field(ty, "window.id", 0);
        self.mac_ok(ty);

        self.mark(failed);
        self.branch(self.macos.destroy, Branch::Call);
        self.mac_msg(Arg::Slot(6), "drain", &[], &[]);
        self.mac_error(
            ty,
            "Could not create the native macOS window or Metal device",
        );
        self.mark(unavailable);
        self.mac_error(ty, "Native macOS graphics are unavailable");
        self.mark(occupied);
        self.mac_error(ty, "A native macOS window is already open");
        self.mark(dimensions);
        self.mac_error(ty, "Window dimensions must be between 1 and 16384 points");
    }

    fn mac_window_poll(&mut self) {
        let ty = self.mac_begin(MacOp::Poll);
        let invalid = self.label();
        let finish = self.label();
        let events = self.label();
        let dispatched = self.label();
        let forward = self.label();
        let keyboard = self.label();
        let pointer = self.label();
        let pointer_forward = self.label();
        let command = self.label();
        let consumed = self.label();
        let close = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.macos.validate, Branch::Call);
        self.branch(invalid, Branch::Zero(0));
        self.mac_imm(KEY, u64::MAX);
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.store(6, 0);
        self.mac_get(CLOSED, 0);
        self.branch(finish, Branch::Nonzero(0));
        self.mac_msg(
            Arg::Ctx(CL_DATE),
            "dateWithTimeIntervalSinceNow:",
            &[],
            &[FArg::Value(0.008)],
        );
        self.store(7, 0);
        self.imm(0, 256);
        self.store(8, 0);

        self.mark(events);
        self.mac_msg(
            Arg::Ctx(APP),
            "nextEventMatchingMask:untilDate:inMode:dequeue:",
            &[
                Arg::Imm(u64::MAX),
                Arg::Slot(7),
                Arg::Ctx(MODE),
                Arg::Imm(1),
            ],
            &[],
        );
        self.store(9, 0);
        self.branch(dispatched, Branch::Zero(0));
        self.imm(0, 0);
        self.store(7, 0); // Remaining events are drained without an idle wait.
        self.mac_msg(Arg::Slot(9), "window", &[], &[]);
        self.mac_get(WINDOW, 1);
        self.cmp(0, 1);
        self.branch(forward, Branch::Cond(1));
        self.mac_msg(Arg::Slot(9), "type", &[], &[]);
        self.store(10, 0);
        self.imm(1, 10);
        self.cmp(0, 1);
        self.branch(keyboard, Branch::Cond(0));
        for kind in [1, 2, 5, 6] {
            self.imm(1, kind);
            self.cmp(0, 1);
            self.branch(pointer, Branch::Cond(0));
        }
        self.branch(forward, Branch::Jump);

        self.mark(keyboard);
        self.mac_msg(Arg::Slot(9), "keyCode", &[], &[]);
        self.imm(1, 0xffff);
        self.and(0, 0, 1);
        self.store(15, 0);
        self.mac_msg(Arg::Slot(9), "modifierFlags", &[], &[]);
        self.imm(1, 1 << 20);
        self.and(0, 0, 1);
        self.branch(command, Branch::Nonzero(0));
        self.load(15);
        self.mac_set(KEY, 0);
        // Deliver every keydown separately; draining more keys here would lose
        // earlier presses before Dunder can observe this frame's single key.
        self.branch(dispatched, Branch::Jump); // Consumed keys must not beep in NSView.
        self.mark(command);
        // Resolve the layout-translated character, rather than US physical keys.
        self.mac_msg(Arg::Slot(9), "charactersIgnoringModifiers", &[], &[]);
        self.store(16, 0);
        self.branch(forward, Branch::Zero(0));
        self.mac_msg(Arg::Slot(16), "length", &[], &[]);
        self.branch(forward, Branch::Zero(0));
        self.mac_msg(Arg::Slot(16), "characterAtIndex:", &[Arg::Imm(0)], &[]);
        self.imm(1, 0xffff);
        self.and(0, 0, 1);
        for character in *b"qQwW" {
            self.imm(1, u64::from(character));
            self.cmp(0, 1);
            self.branch(close, Branch::Cond(0));
        }
        self.branch(forward, Branch::Jump);

        self.mark(pointer);
        self.mac_msg(Arg::Slot(9), "locationInWindow", &[], &[]);
        self.mac_save_fp(11, 0);
        self.mac_save_fp(12, 1);
        self.mac_msg(
            Arg::Ctx(VIEW),
            "convertPoint:fromView:",
            &[Arg::Imm(0)],
            &[FArg::Slot(11), FArg::Slot(12)],
        );
        self.mac_store_fp(POINTER_X, 0);
        self.mac_get(LOGICAL_HEIGHT, 0);
        self.bits_to_fp(0, 0, Type::F64);
        self.emit(0x1e613800); // fsub d0,d0,d1; top-left pointer coordinates
        self.mac_store_fp(POINTER_Y, 0);
        self.load(10);
        self.imm(1, 2);
        self.cmp(0, 1);
        let press = self.label();
        self.branch(press, Branch::Cond(1));
        self.mac_imm(POINTER_DOWN, 0);
        self.branch(pointer_forward, Branch::Jump);
        self.mark(press);
        self.imm(1, 1);
        self.cmp(0, 1);
        self.branch(pointer_forward, Branch::Cond(1));
        // Titlebar/chrome clicks remain native and never begin a content press.
        for (point, size) in [(POINTER_X, LOGICAL_WIDTH), (POINTER_Y, LOGICAL_HEIGHT)] {
            self.mac_get(point, 0);
            self.bits_to_fp(0, 0, Type::F64);
            self.emit(0x1e602008); // fcmp d0,#0.0
            self.branch(pointer_forward, Branch::Cond(4));
            self.mac_get(size, 0);
            self.bits_to_fp(1, 0, Type::F64);
            self.emit(0x1e612000); // fcmp d0,d1
            self.branch(pointer_forward, Branch::Cond(10));
        }
        self.mac_imm(POINTER_DOWN, 1);
        self.branch(pointer_forward, Branch::Jump);

        self.mark(pointer_forward);
        self.mac_msg(Arg::Ctx(APP), "sendEvent:", &[Arg::Slot(9)], &[]);
        self.load(10);
        self.imm(1, 5);
        self.cmp(0, 1);
        // MouseMoved may coalesce, but press/drag/release are meaningful frame
        // boundaries even when AppKit has already queued the complete gesture.
        self.branch(consumed, Branch::Cond(0));
        self.branch(dispatched, Branch::Jump);

        self.mark(close);
        self.mac_imm(CLOSED, 1);
        self.mac_imm(POINTER_DOWN, 0);
        self.mac_msg(Arg::Ctx(WINDOW), "close", &[], &[]);
        self.branch(consumed, Branch::Jump);
        self.mark(forward);
        self.mac_msg(Arg::Ctx(APP), "sendEvent:", &[Arg::Slot(9)], &[]);
        self.mark(consumed);
        self.mac_get(CLOSED, 0);
        self.branch(dispatched, Branch::Nonzero(0));
        self.load(8);
        self.sub_imm(0, 0, 1);
        self.store(8, 0);
        self.branch(events, Branch::Nonzero(0));
        self.mark(dispatched);
        self.mac_msg(Arg::Ctx(APP), "updateWindows", &[], &[]);
        self.branch(self.macos.resize, Branch::Call);
        self.mark(finish);
        self.mac_msg(Arg::Slot(6), "drain", &[], &[]);
        for (field, offset) in [
            ("closed", CLOSED),
            ("width", WIDTH),
            ("height", HEIGHT),
            ("pixel_width", PIXEL_WIDTH),
            ("pixel_height", PIXEL_HEIGHT),
            ("scale", SCALE),
            ("pointer_x", POINTER_X),
            ("pointer_y", POINTER_Y),
            ("pointer_down", POINTER_DOWN),
            ("key", KEY),
            ("resized", RESIZED),
        ] {
            self.mac_get(offset, 0);
            self.mac_field(ty, field, 0);
        }
        self.mac_get(FN_MEDIA_TIME, 16);
        self.emit(0xd63f0200);
        self.mac_get(START_TIME, 0);
        self.bits_to_fp(1, 0, Type::F64);
        self.emit(0x1e613800); // fsub d0,d0,d1
        self.fp_to_bits(0, 0, Type::F64);
        self.mac_field(ty, "time", 0);
        self.mac_imm(RESIZED, 0);
        self.mac_return();
        self.mark(invalid);
        self.imm(0, 1);
        self.mac_field(ty, "closed", 0);
        self.imm(0, u64::MAX);
        self.mac_field(ty, "key", 0);
        self.mac_return();
    }

    fn mac_window_close(&mut self) {
        self.mac_begin(MacOp::Close);
        let done = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.macos.validate, Branch::Call);
        self.branch(done, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.store(6, 0);
        self.branch(self.macos.destroy, Branch::Call);
        self.mac_msg(Arg::Slot(6), "drain", &[], &[]);
        self.mark(done);
        self.imm(0, 0);
        self.epilogue();
    }

    fn mac_window_resize(&mut self) {
        self.mark(self.macos.resize);
        self.prologue(512);
        let done = self.label();
        self.mac_get(VIEW, 0);
        self.branch(done, Branch::Zero(0));
        self.mac_get(LAYER, 0);
        self.branch(done, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(VIEW), "bounds", &[], &[]);
        for reg in 0..4 {
            self.mac_save_fp(20 + reg as usize, reg);
        }
        self.mac_load_fp(22, 0);
        self.mac_store_fp(LOGICAL_WIDTH, 0);
        self.mac_load_fp(23, 0);
        self.mac_store_fp(LOGICAL_HEIGHT, 0);
        self.mac_msg(
            Arg::Ctx(VIEW),
            "convertRectToBacking:",
            &[],
            &[
                FArg::Slot(20),
                FArg::Slot(21),
                FArg::Slot(22),
                FArg::Slot(23),
            ],
        );
        self.mac_save_fp(24, 2);
        self.mac_save_fp(25, 3);
        for (offset, slot) in [
            (WIDTH, 22),
            (HEIGHT, 23),
            (PIXEL_WIDTH, 24),
            (PIXEL_HEIGHT, 25),
        ] {
            self.mac_window_dimension(offset, slot);
        }
        self.mac_msg(Arg::Ctx(WINDOW), "backingScaleFactor", &[], &[]);
        self.mac_save_fp(26, 0);
        self.fp_to_bits(0, 0, Type::F64);
        self.store(28, 0);
        self.mac_get(SCALE, 1);
        self.cmp(0, 1);
        let same = self.label();
        self.branch(same, Branch::Cond(0));
        self.mac_imm(RESIZED, 1);
        self.mark(same);
        self.load(28);
        self.mac_set(SCALE, 0);
        self.mac_msg(Arg::Ctx(CL_TRANSACTION), "begin", &[], &[]);
        self.mac_msg(
            Arg::Ctx(CL_TRANSACTION),
            "setDisableActions:",
            &[Arg::Imm(1)],
            &[],
        );
        self.mac_msg(
            Arg::Ctx(LAYER),
            "setFrame:",
            &[],
            &[
                FArg::Slot(20),
                FArg::Slot(21),
                FArg::Slot(22),
                FArg::Slot(23),
            ],
        );
        self.mac_msg(Arg::Ctx(LAYER), "setContentsScale:", &[], &[FArg::Slot(26)]);
        for (offset, slot) in [(PIXEL_WIDTH, 24), (PIXEL_HEIGHT, 25)] {
            self.mac_get(offset, 0);
            self.emit(0x9e620000); // scvtf d0,x0
            self.mac_save_fp(slot, 0);
        }
        self.mac_msg(
            Arg::Ctx(LAYER),
            "setDrawableSize:",
            &[],
            &[FArg::Slot(24), FArg::Slot(25)],
        );
        self.mac_msg(Arg::Ctx(CL_TRANSACTION), "commit", &[], &[]);
        self.mark(done);
        self.epilogue();
    }

    fn mac_window_dimension(&mut self, offset: u32, slot: usize) {
        self.mac_load_fp(slot, 0);
        self.emit(0x1e644000); // frintn d0,d0: whole pixel/point dimensions
        self.emit(0x9e780000); // fcvtzs x0,d0
        self.store(28, 0);
        self.mac_get(offset, 1);
        self.cmp(0, 1);
        let same = self.label();
        self.branch(same, Branch::Cond(0));
        self.mac_imm(RESIZED, 1);
        self.mark(same);
        self.load(28);
        self.mac_set(offset, 0);
    }

    fn mac_window_destroy(&mut self) {
        self.mark(self.macos.destroy);
        self.prologue(512);
        self.mac_imm(ACTIVE, 0);
        self.mac_imm(CLOSED, 1);
        self.mac_imm(POINTER_DOWN, 0);
        self.mac_msg(Arg::Ctx(APP), "setDelegate:", &[Arg::Imm(0)], &[]);
        let no_window = self.label();
        self.mac_get(WINDOW, 0);
        self.branch(no_window, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "setDelegate:", &[Arg::Imm(0)], &[]);
        self.mac_msg(Arg::Ctx(WINDOW), "close", &[], &[]);
        self.mark(no_window);
        let no_view = self.label();
        self.mac_get(VIEW, 0);
        self.branch(no_view, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(VIEW), "setLayer:", &[Arg::Imm(0)], &[]);
        self.mark(no_view);
        for offset in [
            BUFFER, PIPELINE, QUEUE, DEVICE, WINDOW, VIEW, LAYER, DELEGATE,
        ] {
            let empty = self.label();
            self.mac_get(offset, 0);
            self.branch(empty, Branch::Zero(0));
            self.mac_msg(Arg::Ctx(offset), "release", &[], &[]);
            self.mac_imm(offset, 0);
            self.mark(empty);
        }
        self.mac_imm(VERTEX_COUNT, 0);
        self.epilogue();
    }

    fn mac_window_callbacks(&mut self) {
        self.mark(self.macos.will_close);
        self.prologue(32);
        self.mac_imm(CLOSED, 1);
        self.mac_imm(POINTER_DOWN, 0);
        self.epilogue();

        self.mark(self.macos.resign);
        self.prologue(32);
        self.mac_imm(POINTER_DOWN, 0);
        self.epilogue();

        self.mark(self.macos.should_terminate);
        self.prologue(32);
        self.mac_imm(CLOSED, 1);
        self.mac_imm(POINTER_DOWN, 0);
        self.imm(0, 0); // NSTerminateCancel; Dunder unwinds its own main loop.
        self.epilogue();

        self.mark(self.macos.did_resize);
        self.prologue(512);
        let done = self.label();
        self.mac_get(ACTIVE, 0);
        self.branch(done, Branch::Zero(0));
        self.mac_get(CLOSED, 0);
        self.branch(done, Branch::Nonzero(0));
        self.branch(self.macos.resize, Branch::Call);
        self.mac_get(VERTEX_COUNT, 0);
        self.branch(done, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "isVisible", &[], &[]);
        self.imm(1, 255);
        self.and(0, 0, 1);
        self.branch(done, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "isMiniaturized", &[], &[]);
        self.imm(1, 255);
        self.and(0, 0, 1);
        self.branch(done, Branch::Nonzero(0));
        self.branch(self.macos.render, Branch::Call);
        self.mark(done);
        self.epilogue();
    }
}
