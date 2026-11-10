//! Typed AppKit/Metal boundary emitted as ARM64; no native-language bridge.
//! Only validated generation IDs cross into Dunder. Objective-C callbacks use
//! writable image globals, never Dunder's arena registers.
use super::{Branch, Code, Label};
use crate::{
    check::{Expr, MacOp},
    syntax::Type,
};
mod window;

const FRAME: usize = 512;
const FN_GET_CLASS: u32 = 0;
const FN_SEL: u32 = 8;
const FN_MSG: u32 = 16;
const FN_ALLOC_CLASS: u32 = 24;
const FN_ADD_METHOD: u32 = 32;
const FN_REGISTER_CLASS: u32 = 40;
const FN_MTL_DEVICE: u32 = 48;
const FN_MEDIA_TIME: u32 = 56;
const CL_APP: u32 = 64;
const CL_POOL: u32 = 72;
const CL_STRING: u32 = 80;
const CL_WINDOW: u32 = 88;
const CL_VIEW: u32 = 96;
const CL_LAYER: u32 = 104;
const CL_PIPELINE: u32 = 112;
const CL_PASS: u32 = 120;
const CL_OBJECT: u32 = 128;
const CL_DATE: u32 = 136;
const MODE: u32 = 144;
const APP: u32 = 152;
const ROOT_POOL: u32 = 160;
const DELEGATE_CLASS: u32 = 168;
const GEN: u32 = 176;
const ACTIVE: u32 = 184;
const CLOSED: u32 = 192;
const WINDOW: u32 = 200;
const VIEW: u32 = 208;
const LAYER: u32 = 216;
const DEVICE: u32 = 224;
const QUEUE: u32 = 232;
const PIPELINE: u32 = 240;
const DELEGATE: u32 = 248;
const BUFFER: u32 = 256;
const VERTEX_COUNT: u32 = 264;
const WIDTH: u32 = 272;
const HEIGHT: u32 = 280;
const PIXEL_WIDTH: u32 = 288;
const PIXEL_HEIGHT: u32 = 296;
const SCALE: u32 = 304;
const START_TIME: u32 = 312;
const POINTER_X: u32 = 320;
const POINTER_Y: u32 = 328;
const POINTER_DOWN: u32 = 336;
const KEY: u32 = 344;
const RESIZED: u32 = 352;
const FRAMES: u32 = 360;
const CLEAR_R: u32 = 368;
const CLEAR_G: u32 = 376;
const CLEAR_B: u32 = 384;
const ERROR_CODE: u32 = 392;
const INIT_DONE: u32 = 416;
const LOGICAL_WIDTH: u32 = 424;
const LOGICAL_HEIGHT: u32 = 432;
const CL_TRANSACTION: u32 = 440;
const CL_MENU: u32 = 448;
const CL_MENU_ITEM: u32 = 456;

#[derive(Clone, Copy)]
enum Arg {
    Ctx(u32),
    Slot(usize),
    Imm(u64),
    Address(usize),
}
#[derive(Clone, Copy)]
enum FArg {
    Ctx(u32),
    Slot(usize),
    Value(f64),
}
#[derive(Default)]
pub(super) struct Runtime {
    pub calls: [Label; 4],
    pub init: Label,
    pub validate: Label,
    pub setup: Label,
    pub resize: Label,
    pub render: Label,
    pub destroy: Label,
    pub will_close: Label,
    pub did_resize: Label,
    pub resign: Label,
    pub should_terminate: Label,
}
impl Runtime {
    pub fn new(c: &mut Code<'_>) -> Self {
        Self {
            calls: std::array::from_fn(|_| c.label()),
            init: c.label(),
            validate: c.label(),
            setup: c.label(),
            resize: c.label(),
            render: c.label(),
            destroy: c.label(),
            will_close: c.label(),
            did_resize: c.label(),
            resign: c.label(),
            should_terminate: c.label(),
        }
    }
}
fn index(op: MacOp) -> usize {
    match op {
        MacOp::Open => 0,
        MacOp::Poll => 1,
        MacOp::Draw => 2,
        MacOp::Close => 3,
    }
}

impl Code<'_> {
    pub(super) fn mac_expr(&mut self, op: MacOp, args: &[Expr], storage: Option<usize>) {
        for arg in args {
            self.expr(arg);
            self.push(0);
        }
        for reg in (0..args.len()).rev() {
            self.pop(reg as u32);
        }
        if let Some(storage) = storage {
            self.local_addr(storage);
            self.mov(8, 9);
        }
        self.branch(self.macos.calls[index(op)], Branch::Call);
        if let Some(storage) = storage {
            self.local_addr(storage);
            self.mov(0, 9);
        }
    }
    fn mac_ctx(&mut self, reg: u32) {
        self.global_addr(2, reg);
        self.ldr(reg, reg);
    }
    fn mac_get(&mut self, offset: u32, reg: u32) {
        self.mac_ctx(9);
        self.emit(0xf9400000 | ((offset / 8) << 10) | (9 << 5) | reg);
    }
    fn mac_set(&mut self, offset: u32, reg: u32) {
        assert_ne!(reg, 9);
        self.mac_ctx(9);
        self.emit(0xf9000000 | ((offset / 8) << 10) | (9 << 5) | reg);
    }
    fn mac_imm(&mut self, offset: u32, value: u64) {
        self.imm(0, value);
        self.mac_set(offset, 0);
    }
    fn mac_fn(&mut self, offset: u32) {
        self.mac_get(offset, 16);
        self.emit(0xd63f0200);
    }
    fn mac_cstr(&mut self, text: &str, reg: u32) {
        let label = self.string_literal(&format!("{text}\0"));
        self.branch(label, Branch::Address(reg));
        self.add_imm(reg, reg, 8);
    }
    fn mac_arg(&mut self, arg: Arg, reg: u32) {
        match arg {
            Arg::Ctx(off) => self.mac_get(off, reg),
            Arg::Slot(slot) => {
                self.load(slot);
                if reg != 0 {
                    self.mov(reg, 0);
                }
            }
            Arg::Imm(value) => self.imm(reg, value),
            Arg::Address(slot) => {
                self.local_addr(slot);
                self.mov(reg, 9);
            }
        }
    }
    fn mac_save_fp(&mut self, slot: usize, reg: u32) {
        self.fp_to_bits(0, reg, Type::F64);
        self.store(slot, 0);
    }
    fn mac_store_fp(&mut self, offset: u32, reg: u32) {
        self.fp_to_bits(0, reg, Type::F64);
        self.mac_set(offset, 0);
    }
    fn mac_load_fp(&mut self, slot: usize, reg: u32) {
        self.load(slot);
        self.bits_to_fp(reg, 0, Type::F64);
    }
    fn mac_msg(&mut self, receiver: Arg, selector: &str, args: &[Arg], fp: &[FArg]) {
        assert!(args.len() <= 6 && fp.len() <= 4);
        self.mac_cstr(selector, 0);
        self.mac_fn(FN_SEL);
        self.mov(1, 0);
        for (i, arg) in args.iter().enumerate() {
            self.mac_arg(*arg, i as u32 + 2);
        }
        for (i, arg) in fp.iter().enumerate() {
            match arg {
                FArg::Ctx(off) => {
                    self.mac_get(*off, 0);
                    self.bits_to_fp(i as u32, 0, Type::F64);
                }
                FArg::Slot(slot) => self.mac_load_fp(*slot, i as u32),
                FArg::Value(value) => {
                    self.imm(0, value.to_bits());
                    self.bits_to_fp(i as u32, 0, Type::F64);
                }
            }
        }
        self.mac_arg(receiver, 0);
        self.mac_fn(FN_MSG);
    }
    fn mac_string(&mut self, text: &str) {
        self.mac_cstr(text, 0);
        self.store(60, 0);
        self.mac_msg(
            Arg::Ctx(CL_STRING),
            "stringWithUTF8String:",
            &[Arg::Slot(60)],
            &[],
        );
    }
    fn mac_type(&self, op: MacOp) -> Type {
        let t = self.mac_types.unwrap();
        match op {
            MacOp::Open => t.open,
            MacOp::Poll => t.frame,
            MacOp::Draw => t.draw,
            MacOp::Close => Type::Unit,
        }
    }
    fn mac_begin(&mut self, op: MacOp) -> Type {
        self.mark(self.macos.calls[index(op)]);
        self.prologue(FRAME);
        self.store(0, 8);
        for reg in 0..5 {
            self.store(reg as usize + 1, reg);
        }
        let ty = self.mac_type(op);
        if let Some(bytes) = self.types.storage_bytes(ty) {
            self.load(0);
            self.mov(9, 0);
            self.imm(11, bytes as u64);
            let zero = self.label();
            self.mark(zero);
            self.strb(31, 9);
            self.add_imm(9, 9, 1);
            self.sub_imm(11, 11, 1);
            self.branch(zero, Branch::Nonzero(11));
        }
        if matches!(op, MacOp::Open | MacOp::Draw) {
            let empty = self.string_literal("");
            self.branch(empty, Branch::Address(0));
            self.mac_field(ty, "error", 0);
        }
        ty
    }
    fn mac_field(&mut self, mut ty: Type, path: &str, reg: u32) {
        let mut offset = 0;
        for name in path.split('.') {
            let field = self
                .types
                .record(ty)
                .unwrap()
                .fields
                .iter()
                .find(|f| f.name == name)
                .unwrap();
            offset += field.offset;
            ty = field.ty;
        }
        self.local_addr(0);
        self.ldr(9, 9);
        self.imm(10, offset as u64);
        self.add(9, 9, 10);
        self.typed_store_reg(ty, 9, reg);
    }
    fn mac_return(&mut self) {
        self.load(0);
        self.epilogue();
    }
    fn mac_ok(&mut self, ty: Type) {
        self.imm(0, 1);
        self.mac_field(ty, "ok", 0);
        self.mac_return();
    }
    fn mac_error(&mut self, ty: Type, message: &str) {
        let label = self.string_literal(message);
        self.branch(label, Branch::Address(0));
        self.mac_field(ty, "error", 0);
        self.imm(0, 0);
        self.mac_field(ty, "ok", 0);
        self.mac_return();
    }
    pub(super) fn mac_runtime(&mut self) {
        self.mac_init_runtime();
        self.mac_validate_runtime();
        self.mac_setup_runtime();
        self.mac_draw_runtime();
        self.mac_render_runtime();
        self.mac_window_runtime();
    }
    fn mac_validate_runtime(&mut self) {
        self.mark(self.macos.validate);
        self.mov(1, 0);
        self.mac_ctx(0);
        let invalid = self.label();
        self.branch(invalid, Branch::Zero(0));
        self.mov(2, 0);
        self.mac_get(ACTIVE, 3);
        self.branch(invalid, Branch::Zero(3));
        self.mac_get(GEN, 3);
        self.cmp(1, 3);
        self.branch(invalid, Branch::Cond(1));
        self.mov(0, 2);
        self.ret();
        self.mark(invalid);
        self.imm(0, 0);
        self.ret();
    }
    fn mac_init_runtime(&mut self) {
        self.mark(self.macos.init);
        self.prologue(FRAME);
        let mapped = self.label();
        let ready = self.label();
        let failed = self.label();
        self.mac_ctx(0);
        self.branch(mapped, Branch::Nonzero(0));
        self.imm(0, 0);
        self.imm(1, 65536);
        self.imm(2, 3);
        self.imm(3, 0x1002);
        self.imm(4, u64::MAX);
        self.imm(5, 0);
        self.imm(16, 0x20000c5);
        self.emit(0xd4001001);
        self.branch(failed, Branch::Cond(2));
        self.global_addr(2, 9);
        self.str(0, 9);
        self.mark(mapped);
        self.mac_get(INIT_DONE, 0);
        self.branch(ready, Branch::Nonzero(0));
        for (slot, path) in [
            (0, "/usr/lib/libobjc.A.dylib"),
            (1, "/System/Library/Frameworks/AppKit.framework/AppKit"),
            (
                2,
                "/System/Library/Frameworks/QuartzCore.framework/QuartzCore",
            ),
            (3, "/System/Library/Frameworks/Metal.framework/Metal"),
        ] {
            self.mac_cstr(path, 0);
            self.imm(1, 6);
            self.import_call(0);
            self.branch(failed, Branch::Zero(0));
            self.store(slot, 0);
        }
        for (lib, name, off) in [
            (0, "objc_getClass", FN_GET_CLASS),
            (0, "sel_registerName", FN_SEL),
            (0, "objc_msgSend", FN_MSG),
            (0, "objc_allocateClassPair", FN_ALLOC_CLASS),
            (0, "class_addMethod", FN_ADD_METHOD),
            (0, "objc_registerClassPair", FN_REGISTER_CLASS),
            (3, "MTLCreateSystemDefaultDevice", FN_MTL_DEVICE),
            (2, "CACurrentMediaTime", FN_MEDIA_TIME),
        ] {
            self.mac_cstr(name, 1);
            self.load(lib);
            self.import_call(1);
            self.branch(failed, Branch::Zero(0));
            self.mac_set(off, 0);
        }
        for (name, off) in [
            ("NSApplication", CL_APP),
            ("NSAutoreleasePool", CL_POOL),
            ("NSString", CL_STRING),
            ("NSWindow", CL_WINDOW),
            ("NSView", CL_VIEW),
            ("CAMetalLayer", CL_LAYER),
            ("MTLRenderPipelineDescriptor", CL_PIPELINE),
            ("MTLRenderPassDescriptor", CL_PASS),
            ("NSObject", CL_OBJECT),
            ("NSDate", CL_DATE),
            ("CATransaction", CL_TRANSACTION),
            ("NSMenu", CL_MENU),
            ("NSMenuItem", CL_MENU_ITEM),
        ] {
            self.mac_cstr(name, 0);
            self.mac_fn(FN_GET_CLASS);
            self.branch(failed, Branch::Zero(0));
            self.mac_set(off, 0);
        }
        self.mac_cstr("NSDefaultRunLoopMode", 1);
        self.load(1);
        self.import_call(1);
        self.branch(failed, Branch::Zero(0));
        self.ldr(0, 0);
        self.mac_set(MODE, 0);
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.mac_set(ROOT_POOL, 0);
        self.mac_msg(Arg::Ctx(CL_APP), "sharedApplication", &[], &[]);
        self.store(5, 0);
        self.mac_msg(Arg::Slot(5), "retain", &[], &[]);
        self.mac_set(APP, 0);
        self.mac_msg(Arg::Ctx(APP), "setActivationPolicy:", &[Arg::Imm(0)], &[]);
        self.mac_cstr("DunderNativeWindowDelegateV1", 1);
        self.mac_get(CL_OBJECT, 0);
        self.imm(2, 0);
        self.mac_fn(FN_ALLOC_CLASS);
        self.branch(failed, Branch::Zero(0));
        self.mac_set(DELEGATE_CLASS, 0);
        for (selector, label, encoding) in [
            ("windowWillClose:", self.macos.will_close, "v@:@"),
            ("windowDidResize:", self.macos.did_resize, "v@:@"),
            (
                "windowDidChangeBackingProperties:",
                self.macos.did_resize,
                "v@:@",
            ),
            ("windowDidResignKey:", self.macos.resign, "v@:@"),
            ("applicationDidResignActive:", self.macos.resign, "v@:@"),
            (
                "applicationShouldTerminate:",
                self.macos.should_terminate,
                "Q@:@",
            ),
        ] {
            self.mac_cstr(selector, 0);
            self.mac_fn(FN_SEL);
            self.mov(1, 0);
            self.branch(label, Branch::Address(2));
            self.mac_cstr(encoding, 3);
            self.mac_get(DELEGATE_CLASS, 0);
            self.mac_fn(FN_ADD_METHOD);
        }
        self.mac_get(DELEGATE_CLASS, 0);
        self.mac_fn(FN_REGISTER_CLASS);
        self.mac_menus();
        self.mac_msg(Arg::Ctx(APP), "finishLaunching", &[], &[]);
        self.mac_imm(INIT_DONE, 1);
        self.mark(ready);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
    }
    fn mac_menus(&mut self) {
        self.mac_msg(Arg::Ctx(CL_MENU), "new", &[], &[]);
        self.store(10, 0);
        for (title, items) in [
            ("Dunder", vec![("Quit Dunder", "terminate:", "q")]),
            (
                "Window",
                vec![
                    ("Minimize", "performMiniaturize:", "m"),
                    ("Close Window", "performClose:", "w"),
                ],
            ),
        ] {
            self.mac_msg(Arg::Ctx(CL_MENU_ITEM), "new", &[], &[]);
            self.store(11, 0);
            self.mac_msg(Arg::Ctx(CL_MENU), "new", &[], &[]);
            self.store(12, 0);
            self.mac_string(title);
            self.store(13, 0);
            self.mac_msg(Arg::Slot(11), "setTitle:", &[Arg::Slot(13)], &[]);
            for (item, action, key) in items {
                self.mac_string(item);
                self.store(14, 0);
                self.mac_string(key);
                self.store(15, 0);
                self.mac_cstr(action, 0);
                self.mac_fn(FN_SEL);
                self.store(16, 0);
                self.mac_msg(Arg::Ctx(CL_MENU_ITEM), "alloc", &[], &[]);
                self.store(17, 0);
                self.mac_msg(
                    Arg::Slot(17),
                    "initWithTitle:action:keyEquivalent:",
                    &[Arg::Slot(14), Arg::Slot(16), Arg::Slot(15)],
                    &[],
                );
                self.store(17, 0);
                self.mac_msg(Arg::Slot(12), "addItem:", &[Arg::Slot(17)], &[]);
                self.mac_msg(Arg::Slot(17), "release", &[], &[]);
            }
            self.mac_msg(Arg::Slot(11), "setSubmenu:", &[Arg::Slot(12)], &[]);
            self.mac_msg(Arg::Slot(10), "addItem:", &[Arg::Slot(11)], &[]);
            self.mac_msg(Arg::Slot(12), "release", &[], &[]);
            self.mac_msg(Arg::Slot(11), "release", &[], &[]);
        }
        self.mac_msg(Arg::Ctx(APP), "setMainMenu:", &[Arg::Slot(10)], &[]);
        self.mac_msg(Arg::Slot(10), "release", &[], &[]);
    }
    fn mac_setup_runtime(&mut self) {
        self.mark(self.macos.setup);
        self.prologue(FRAME);
        let failed = self.label();
        self.mac_fn(FN_MTL_DEVICE);
        self.mac_set(DEVICE, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(DEVICE), "newCommandQueue", &[], &[]);
        self.mac_set(QUEUE, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_string(SHADER);
        self.store(1, 0);
        self.imm(0, 0);
        self.store(2, 0);
        self.mac_msg(
            Arg::Ctx(DEVICE),
            "newLibraryWithSource:options:error:",
            &[Arg::Slot(1), Arg::Imm(0), Arg::Address(2)],
            &[],
        );
        self.store(3, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_string("vertex_main");
        self.store(4, 0);
        self.mac_msg(Arg::Slot(3), "newFunctionWithName:", &[Arg::Slot(4)], &[]);
        self.store(5, 0);
        self.mac_string("fragment_main");
        self.store(4, 0);
        self.mac_msg(Arg::Slot(3), "newFunctionWithName:", &[Arg::Slot(4)], &[]);
        self.store(6, 0);
        self.mac_msg(Arg::Ctx(CL_PIPELINE), "new", &[], &[]);
        self.store(7, 0);
        self.mac_msg(Arg::Slot(7), "setVertexFunction:", &[Arg::Slot(5)], &[]);
        self.mac_msg(Arg::Slot(7), "setFragmentFunction:", &[Arg::Slot(6)], &[]);
        self.mac_msg(Arg::Slot(7), "colorAttachments", &[], &[]);
        self.store(8, 0);
        self.mac_msg(
            Arg::Slot(8),
            "objectAtIndexedSubscript:",
            &[Arg::Imm(0)],
            &[],
        );
        self.store(8, 0);
        for (selector, value) in [
            ("setPixelFormat:", 80),
            ("setBlendingEnabled:", 1),
            ("setSourceRGBBlendFactor:", 4),
            ("setDestinationRGBBlendFactor:", 5),
            ("setSourceAlphaBlendFactor:", 1),
            ("setDestinationAlphaBlendFactor:", 5),
        ] {
            self.mac_msg(Arg::Slot(8), selector, &[Arg::Imm(value)], &[]);
        }
        self.imm(0, 0);
        self.store(2, 0);
        self.mac_msg(
            Arg::Ctx(DEVICE),
            "newRenderPipelineStateWithDescriptor:error:",
            &[Arg::Slot(7), Arg::Address(2)],
            &[],
        );
        self.mac_set(PIPELINE, 0);
        for slot in [7, 6, 5, 3] {
            self.mac_msg(Arg::Slot(slot), "release", &[], &[]);
        }
        self.mac_get(PIPELINE, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_imm(ERROR_CODE, 0);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.mac_imm(ERROR_CODE, 1);
        self.imm(0, 0);
        self.epilogue();
    }
    fn mac_draw_runtime(&mut self) {
        let ty = self.mac_begin(MacOp::Draw);
        let invalid = self.label();
        let closed = self.label();
        let geometry = self.label();
        let failed = self.label();
        self.load(1);
        self.ldr(0, 0);
        self.branch(self.macos.validate, Branch::Call);
        self.branch(invalid, Branch::Zero(0));
        self.mac_get(FRAMES, 0);
        self.mac_field(ty, "frames", 0);
        self.mac_get(CLOSED, 0);
        self.branch(closed, Branch::Nonzero(0));
        self.load(2);
        self.add_imm(0, 0, 8);
        self.ldr(0, 0);
        self.store(6, 0);
        self.imm(1, 8);
        self.emit(0x9ac10802);
        self.emit(0x9b018040);
        self.branch(geometry, Branch::Nonzero(0));
        self.load(6);
        self.imm(1, 800000);
        self.cmp(0, 1);
        self.branch(geometry, Branch::Cond(8));
        self.imm(1, 8);
        self.emit(0x9ac10800);
        self.store(7, 0);
        self.imm(1, 3);
        self.emit(0x9ac10802);
        self.emit(0x9b018040);
        self.branch(geometry, Branch::Nonzero(0));
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.store(8, 0);
        self.mac_get(BUFFER, 0);
        self.store(9, 0);
        let empty = self.label();
        let uploaded = self.label();
        self.load(6);
        self.branch(empty, Branch::Zero(0));
        self.load(2);
        self.ldr(0, 0);
        self.store(10, 0);
        self.load(6);
        self.lsl(0, 0, 2);
        self.store(11, 0);
        self.mac_msg(
            Arg::Ctx(DEVICE),
            "newBufferWithBytes:length:options:",
            &[Arg::Slot(10), Arg::Slot(11), Arg::Imm(0)],
            &[],
        );
        self.branch(failed, Branch::Zero(0));
        self.mac_set(BUFFER, 0);
        self.branch(uploaded, Branch::Jump);
        self.mark(empty);
        self.mac_imm(BUFFER, 0);
        self.mark(uploaded);
        self.mac_msg(Arg::Slot(9), "release", &[], &[]);
        self.load(7);
        self.mac_set(VERTEX_COUNT, 0);
        for (slot, off) in [(3, CLEAR_R), (4, CLEAR_G), (5, CLEAR_B)] {
            self.load(slot);
            self.mac_set(off, 0);
        }
        self.branch(self.macos.render, Branch::Call);
        self.store(12, 0);
        self.mac_msg(Arg::Slot(8), "drain", &[], &[]);
        self.load(12);
        let gpu_error = self.label();
        self.branch(gpu_error, Branch::Zero(0));
        self.mac_get(FRAMES, 0);
        self.mac_field(ty, "frames", 0);
        self.mac_ok(ty);
        self.mark(failed);
        self.mac_msg(Arg::Slot(8), "drain", &[], &[]);
        self.mac_error(ty, "Metal vertex buffer allocation failed");
        self.mark(gpu_error);
        self.mac_error(ty, "Metal command buffer failed");
        self.mark(geometry);
        self.mac_error(
            ty,
            "vertices must be a triangle list of eight f32 values per vertex (max 100000 vertices)",
        );
        self.mark(closed);
        self.mac_error(ty, "window is closed");
        self.mark(invalid);
        self.mac_error(ty, "invalid or expired window handle");
    }
    fn mac_render_runtime(&mut self) {
        self.mark(self.macos.render);
        self.prologue(FRAME);
        let skip = self.label();
        let failed = self.label();
        let complete = self.label();
        self.mac_get(ACTIVE, 0);
        self.branch(skip, Branch::Zero(0));
        self.mac_get(CLOSED, 0);
        self.branch(skip, Branch::Nonzero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "isVisible", &[], &[]);
        self.emit(0x53001c00);
        self.branch(skip, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(WINDOW), "isMiniaturized", &[], &[]);
        self.emit(0x53001c00);
        self.branch(skip, Branch::Nonzero(0));
        self.mac_get(PIXEL_WIDTH, 0);
        self.branch(skip, Branch::Zero(0));
        self.mac_get(PIXEL_HEIGHT, 0);
        self.branch(skip, Branch::Zero(0));
        self.mac_msg(Arg::Ctx(CL_POOL), "new", &[], &[]);
        self.store(1, 0);
        self.mac_msg(Arg::Ctx(LAYER), "nextDrawable", &[], &[]);
        self.store(2, 0);
        self.branch(complete, Branch::Zero(0));
        self.mac_msg(Arg::Slot(2), "texture", &[], &[]);
        self.store(3, 0);
        self.mac_msg(Arg::Ctx(CL_PASS), "renderPassDescriptor", &[], &[]);
        self.store(4, 0);
        self.mac_msg(Arg::Slot(4), "colorAttachments", &[], &[]);
        self.store(5, 0);
        self.mac_msg(
            Arg::Slot(5),
            "objectAtIndexedSubscript:",
            &[Arg::Imm(0)],
            &[],
        );
        self.store(5, 0);
        self.mac_msg(Arg::Slot(5), "setTexture:", &[Arg::Slot(3)], &[]);
        self.mac_msg(Arg::Slot(5), "setLoadAction:", &[Arg::Imm(2)], &[]);
        self.mac_msg(Arg::Slot(5), "setStoreAction:", &[Arg::Imm(1)], &[]);
        self.mac_msg(
            Arg::Slot(5),
            "setClearColor:",
            &[],
            &[
                FArg::Ctx(CLEAR_R),
                FArg::Ctx(CLEAR_G),
                FArg::Ctx(CLEAR_B),
                FArg::Value(1.0),
            ],
        );
        self.mac_msg(Arg::Ctx(QUEUE), "commandBuffer", &[], &[]);
        self.store(6, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(
            Arg::Slot(6),
            "renderCommandEncoderWithDescriptor:",
            &[Arg::Slot(4)],
            &[],
        );
        self.store(7, 0);
        self.branch(failed, Branch::Zero(0));
        self.mac_msg(
            Arg::Slot(7),
            "setRenderPipelineState:",
            &[Arg::Ctx(PIPELINE)],
            &[],
        );
        self.mac_msg(Arg::Slot(7), "setCullMode:", &[Arg::Imm(0)], &[]);
        let no_vertices = self.label();
        self.mac_get(VERTEX_COUNT, 0);
        self.branch(no_vertices, Branch::Zero(0));
        self.mac_msg(
            Arg::Slot(7),
            "setVertexBuffer:offset:atIndex:",
            &[Arg::Ctx(BUFFER), Arg::Imm(0), Arg::Imm(0)],
            &[],
        );
        self.mac_msg(
            Arg::Slot(7),
            "drawPrimitives:vertexStart:vertexCount:",
            &[Arg::Imm(3), Arg::Imm(0), Arg::Ctx(VERTEX_COUNT)],
            &[],
        );
        self.mark(no_vertices);
        self.mac_msg(Arg::Slot(7), "endEncoding", &[], &[]);
        self.mac_msg(Arg::Slot(6), "presentDrawable:", &[Arg::Slot(2)], &[]);
        self.mac_msg(Arg::Slot(6), "commit", &[], &[]);
        self.mac_msg(Arg::Slot(6), "waitUntilCompleted", &[], &[]);
        self.mac_msg(Arg::Slot(6), "status", &[], &[]);
        self.imm(1, 4);
        self.cmp(0, 1);
        self.branch(failed, Branch::Cond(1));
        self.mac_msg(Arg::Slot(6), "error", &[], &[]);
        self.branch(failed, Branch::Nonzero(0));
        self.mac_get(FRAMES, 0);
        self.add_imm(0, 0, 1);
        self.mac_set(FRAMES, 0);
        self.mac_imm(ERROR_CODE, 0);
        self.mark(complete);
        self.mac_msg(Arg::Slot(1), "drain", &[], &[]);
        self.mark(skip);
        self.imm(0, 1);
        self.epilogue();
        self.mark(failed);
        self.mac_imm(ERROR_CODE, 2);
        self.mac_msg(Arg::Slot(1), "drain", &[], &[]);
        self.imm(0, 0);
        self.epilogue();
    }
}
const SHADER: &str = r#"#include <metal_stdlib>
using namespace metal;
struct Vertex { float4 position; float4 color; };
struct Output { float4 position [[position]]; float4 color; };
vertex Output vertex_main(uint index [[vertex_id]], constant Vertex* vertices [[buffer(0)]]) {
    Output value; value.position = vertices[index].position; value.color = vertices[index].color; return value;
}
fragment float4 fragment_main(Output value [[stage_in]]) { return value.color; }
"#;
