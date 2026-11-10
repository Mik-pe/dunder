//! macOS/arm64 filesystem boundary. Constants/layout follow Apple's XNU and
//! macOS SDK: stat64 is 144 bytes, st_mode at 4, st_size at 96. No libc calls.
use super::{Branch, Code, Label};
use crate::{
    check::{Expr, SysOp},
    syntax::Type,
};

const FRAME: usize = 2448;
const PATH1: usize = 143; // FP-1152, 1024 bytes
const PATH2: usize = 271; // FP-2176, 1024 bytes
const STAT: usize = 303; // FP-2432, 256 bytes
const MAX_READ: u64 = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Runtime {
    calls: [Label; 9],
    path: Label,
    utf8: Label,
}
impl Runtime {
    pub fn new(code: &mut Code<'_>) -> Self {
        Self {
            calls: std::array::from_fn(|_| code.label()),
            path: code.label(),
            utf8: code.label(),
        }
    }
}
fn index(op: SysOp) -> usize {
    match op {
        SysOp::ReadText => 0,
        SysOp::WriteText => 1,
        SysOp::AppendText => 2,
        SysOp::Mkdir => 3,
        SysOp::Rename => 4,
        SysOp::RemoveFile => 5,
        SysOp::RemoveDir => 6,
        SysOp::Metadata => 7,
        SysOp::Pid => 8,
    }
}
impl Code<'_> {
    pub(super) fn sys_expr(&mut self, op: SysOp, args: &[Expr], storage: Option<usize>) {
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
        self.branch(self.sys.calls[index(op)], Branch::Call);
        if let Some(storage) = storage {
            self.local_addr(storage);
            self.mov(0, 9);
        }
    }
    fn syscall(&mut self, number: u64) {
        self.imm(16, 0x2000000 + number);
        self.emit(0xd4001001);
    }
    fn sys_type(&self, op: SysOp) -> Type {
        let types = self.sys_types.unwrap();
        match op {
            SysOp::ReadText => types.text,
            SysOp::Metadata => types.metadata,
            _ => types.status,
        }
    }
    fn sys_field(&mut self, ty: Type, name: &str, reg: u32) {
        let field = self
            .types
            .record(ty)
            .unwrap()
            .fields
            .iter()
            .find(|f| f.name == name)
            .unwrap();
        let (offset, field_ty) = (field.offset, field.ty);
        self.local_addr(0);
        self.emit(0xf9400129); // result address
        self.imm(10, offset as u64);
        self.add(9, 9, 10);
        self.typed_store_reg(field_ty, 9, reg);
    }
    fn sys_begin(&mut self, op: SysOp) -> Type {
        self.mark(self.sys.calls[index(op)]);
        self.prologue(FRAME);
        self.store(0, 8);
        self.store(1, 0);
        self.store(2, 1);
        let ty = self.sys_type(op);
        self.local_addr(0);
        self.emit(0xf9400129);
        self.imm(11, self.types.storage_bytes(ty).unwrap() as u64);
        let zero = self.label();
        self.mark(zero);
        self.strb(31, 9);
        self.add_imm(9, 9, 1);
        self.sub_imm(11, 11, 1);
        self.branch(zero, Branch::Nonzero(11));
        if op == SysOp::ReadText {
            let empty = self.string_literal("");
            self.branch(empty, Branch::Address(0));
            self.sys_field(ty, "value", 0);
        }
        ty
    }
    // errno in x0; the caller-owned result is always completely initialized.
    fn sys_finish(&mut self, ty: Type) {
        self.sys_field(ty, "error", 0);
        self.cmp(0, 31);
        self.emit(0x9a9f17e1); // cset x1,eq
        self.sys_field(ty, "ok", 1);
        self.load(0);
        self.epilogue();
    }
    fn sys_path(&mut self, source: usize, buffer: usize, failed: Label) {
        self.load(source);
        self.push(0);
        self.local_addr(buffer);
        self.mov(1, 9);
        self.pop(0);
        self.branch(self.sys.path, Branch::Call);
        self.branch(failed, Branch::Nonzero(0));
    }
    pub(super) fn sys_runtime(&mut self) {
        if self.sys_types.is_none() {
            return;
        }
        self.sys_path_runtime();
        self.sys_utf8_runtime();
        self.mark(self.sys.calls[index(SysOp::Pid)]);
        self.syscall(20);
        self.ret();
        for (op, number) in [
            (SysOp::Mkdir, 136),
            (SysOp::Rename, 128),
            (SysOp::RemoveFile, 10),
            (SysOp::RemoveDir, 137),
        ] {
            let ty = self.sys_begin(op);
            let done = self.label();
            self.sys_path(1, PATH1, done);
            if op == SysOp::Rename {
                self.sys_path(2, PATH2, done);
                self.local_addr(PATH2);
                self.mov(1, 9);
            } else if op == SysOp::Mkdir {
                self.imm(1, 0o700);
            }
            self.local_addr(PATH1);
            self.mov(0, 9);
            self.syscall(number);
            self.branch(done, Branch::Cond(2));
            self.imm(0, 0);
            self.mark(done);
            self.sys_finish(ty);
        }
        let ty = self.sys_begin(SysOp::Metadata);
        let done = self.label();
        self.sys_path(1, PATH1, done);
        self.local_addr(PATH1);
        self.mov(0, 9);
        self.local_addr(STAT);
        self.mov(1, 9);
        self.syscall(338);
        self.branch(done, Branch::Cond(2));
        self.local_addr(STAT);
        self.emit(0xf9403120); // ldr x0,[x9,#96]
        self.sys_field(ty, "size", 0);
        self.local_addr(STAT);
        self.emit(0x79400920); // ldrh w0,[x9,#4]
        self.imm(1, 0xf000);
        self.and(0, 0, 1);
        self.mov(2, 0);
        for (kind, field) in [(0x8000, "is_file"), (0x4000, "is_dir")] {
            self.imm(1, kind);
            self.cmp(2, 1);
            self.emit(0x9a9f17e0);
            self.sys_field(ty, field, 0);
        }
        self.imm(0, 0);
        self.mark(done);
        self.sys_finish(ty);
        self.sys_io(SysOp::ReadText);
        self.sys_io(SysOp::WriteText);
        self.sys_io(SysOp::AppendText);
    }
    fn sys_io(&mut self, op: SysOp) {
        let reading = op == SysOp::ReadText;
        let ty = self.sys_begin(op);
        let done = self.label();
        let opened_error = self.label();
        let close = self.label();
        let close_done = self.label();
        self.sys_path(1, PATH1, done);
        let open = self.label();
        self.mark(open);
        self.local_addr(PATH1);
        self.mov(0, 9);
        let flags = 0x01000004
            | if reading {
                0
            } else {
                1 | 0x200 | if op == SysOp::AppendText { 8 } else { 0 }
            };
        self.imm(1, flags);
        self.imm(2, 0o600);
        self.syscall(5);
        let opened = self.label();
        self.branch(opened, Branch::Cond(3));
        self.imm(1, 4);
        self.cmp(0, 1);
        self.branch(open, Branch::Cond(0));
        self.branch(done, Branch::Jump);
        self.mark(opened);
        self.store(3, 0);
        let stat = self.label();
        self.mark(stat);
        self.load(3);
        self.local_addr(STAT);
        self.mov(1, 9);
        self.syscall(339);
        let stated = self.label();
        self.branch(stated, Branch::Cond(3));
        self.imm(1, 4);
        self.cmp(0, 1);
        self.branch(stat, Branch::Cond(0));
        self.branch(opened_error, Branch::Jump);
        self.mark(stated);
        self.local_addr(STAT);
        self.emit(0x79400920);
        self.imm(1, 0xf000);
        self.and(0, 0, 1);
        let regular = self.label();
        let special = self.label();
        self.imm(1, 0x8000);
        self.cmp(0, 1);
        self.branch(regular, Branch::Cond(0));
        self.imm(1, 0x4000);
        self.cmp(0, 1);
        self.branch(special, Branch::Cond(1));
        self.imm(0, 21);
        self.branch(opened_error, Branch::Jump);
        self.mark(special);
        self.imm(0, 45);
        self.branch(opened_error, Branch::Jump);
        self.mark(regular);
        if reading {
            self.local_addr(STAT);
            self.emit(0xf9403120);
            self.imm(1, MAX_READ);
            self.cmp(0, 1);
            let fits = self.label();
            self.branch(fits, Branch::Cond(9));
            self.imm(0, 27);
            self.branch(opened_error, Branch::Jump);
            self.mark(fits);
            self.store(7, 0);
            self.branch(self.strings.try_alloc, Branch::Call);
            let allocated = self.label();
            self.branch(allocated, Branch::Nonzero(0));
            self.imm(0, 12);
            self.branch(opened_error, Branch::Jump);
            self.mark(allocated);
            self.store(5, 0);
        } else {
            self.load(2);
            self.store(5, 0);
            self.ldr(0, 0);
            self.store(7, 0);
            if op == SysOp::WriteText {
                let truncate = self.label();
                let truncated = self.label();
                self.mark(truncate);
                self.load(3);
                self.imm(1, 0);
                self.syscall(201);
                self.branch(truncated, Branch::Cond(3));
                self.imm(1, 4);
                self.cmp(0, 1);
                self.branch(truncate, Branch::Cond(0));
                self.branch(opened_error, Branch::Jump);
                self.mark(truncated);
            }
        }
        self.imm(0, 0);
        self.store(6, 0);
        let loop_head = self.label();
        let complete = self.label();
        let attempt = self.label();
        let advanced = self.label();
        self.mark(loop_head);
        self.load(6);
        self.push(0);
        self.load(7);
        self.pop(1);
        self.cmp(1, 0);
        self.branch(complete, Branch::Cond(0));
        self.mark(attempt);
        self.load(3);
        self.push(0);
        self.load(5);
        self.add_imm(0, 0, 8);
        self.push(0);
        self.load(6);
        self.pop(1);
        self.add(1, 1, 0);
        self.load(7);
        self.mov(2, 0);
        self.load(6);
        self.sub(2, 2, 0);
        self.pop(0);
        self.syscall(if reading { 3 } else { 4 });
        self.branch(advanced, Branch::Cond(3));
        self.imm(1, 4);
        self.cmp(0, 1);
        self.branch(attempt, Branch::Cond(0));
        self.branch(opened_error, Branch::Jump);
        self.mark(advanced);
        if reading {
            self.branch(complete, Branch::Zero(0));
        } else {
            let progress = self.label();
            self.branch(progress, Branch::Nonzero(0));
            self.imm(0, 5);
            self.branch(opened_error, Branch::Jump);
            self.mark(progress);
        }
        self.push(0);
        self.load(6);
        self.pop(1);
        self.add(0, 0, 1);
        self.store(6, 0);
        self.branch(loop_head, Branch::Jump);
        self.mark(complete);
        if reading {
            // Probe EOF: a file that grew beyond its measured size is EAGAIN,
            // never a silently truncated successful read.
            self.load(6);
            self.push(0);
            self.load(7);
            self.pop(1);
            self.cmp(0, 1);
            let validate = self.label();
            self.branch(validate, Branch::Cond(1));
            let probe = self.label();
            let probed = self.label();
            self.mark(probe);
            self.load(3);
            self.local_addr(14);
            self.mov(1, 9);
            self.imm(2, 1);
            self.syscall(3);
            self.branch(probed, Branch::Cond(3));
            self.imm(1, 4);
            self.cmp(0, 1);
            self.branch(probe, Branch::Cond(0));
            self.branch(opened_error, Branch::Jump);
            self.mark(probed);
            self.branch(validate, Branch::Zero(0));
            self.imm(0, 35);
            self.branch(opened_error, Branch::Jump);
            self.mark(validate);
            self.load(5);
            self.mov(1, 0);
            self.load(6);
            self.str(0, 1);
            self.load(5);
            self.branch(self.sys.utf8, Branch::Call);
            self.branch(opened_error, Branch::Nonzero(0));
        }
        self.imm(0, 0);
        self.mark(opened_error);
        self.store(4, 0);
        self.branch(close, Branch::Jump);
        self.mark(close);
        self.load(3);
        self.syscall(6);
        self.branch(close_done, Branch::Cond(3));
        self.push(0);
        self.load(4);
        let earlier = self.label();
        self.branch(earlier, Branch::Nonzero(0));
        self.pop(0);
        self.store(4, 0);
        self.branch(close_done, Branch::Jump);
        self.mark(earlier);
        self.pop(0);
        self.mark(close_done);
        if reading {
            self.load(4);
            let failed = self.label();
            self.branch(failed, Branch::Nonzero(0));
            self.load(5);
            self.sys_field(ty, "value", 0);
            self.mark(failed);
        }
        self.load(4);
        self.mark(done);
        self.sys_finish(ty);
    }
    fn sys_path_runtime(&mut self) {
        self.mark(self.sys.path);
        self.ldr(2, 0);
        self.imm(3, 1024);
        self.cmp(2, 3);
        let long = self.label();
        let invalid = self.label();
        let again = self.label();
        let end = self.label();
        self.branch(long, Branch::Cond(2));
        self.add_imm(0, 0, 8);
        self.mark(again);
        self.branch(end, Branch::Zero(2));
        self.ldrb(3, 0);
        self.branch(invalid, Branch::Zero(3));
        self.strb(3, 1);
        self.add_imm(0, 0, 1);
        self.add_imm(1, 1, 1);
        self.sub_imm(2, 2, 1);
        self.branch(again, Branch::Jump);
        self.mark(end);
        self.strb(31, 1);
        self.imm(0, 0);
        self.ret();
        self.mark(long);
        self.imm(0, 63);
        self.ret();
        self.mark(invalid);
        self.imm(0, 22);
        self.ret();
    }
    fn sys_utf8_runtime(&mut self) {
        self.mark(self.sys.utf8);
        self.prologue(0);
        self.ldr(7, 0);
        self.add_imm(6, 0, 8);
        self.add(7, 6, 7);
        let head = self.label();
        let ascii = self.label();
        let invalid = self.label();
        let done = self.label();
        self.mark(head);
        self.cmp(6, 7);
        self.branch(done, Branch::Cond(0));
        self.ldrb(2, 6);
        self.imm(3, 128);
        self.cmp(2, 3);
        self.branch(ascii, Branch::Cond(3));
        self.imm(3, 194);
        self.cmp(2, 3);
        self.branch(invalid, Branch::Cond(3));
        self.imm(3, 244);
        self.cmp(2, 3);
        self.branch(invalid, Branch::Cond(8));
        self.imm(8, 2);
        self.imm(3, 224);
        self.cmp(2, 3);
        let width = self.label();
        self.branch(width, Branch::Cond(3));
        self.imm(8, 3);
        self.imm(3, 240);
        self.cmp(2, 3);
        self.branch(width, Branch::Cond(3));
        self.imm(8, 4);
        self.mark(width);
        self.add(3, 6, 8);
        self.cmp(3, 7);
        self.branch(invalid, Branch::Cond(8));
        self.imm(3, 1);
        let continuation = self.label();
        self.mark(continuation);
        self.add(4, 6, 3);
        self.ldrb(4, 4);
        self.imm(5, 128);
        self.cmp(4, 5);
        self.branch(invalid, Branch::Cond(3));
        self.imm(5, 191);
        self.cmp(4, 5);
        self.branch(invalid, Branch::Cond(8));
        self.add_imm(3, 3, 1);
        self.cmp(3, 8);
        self.branch(continuation, Branch::Cond(3));
        self.mov(0, 6);
        self.branch(self.strings.decode, Branch::Call);
        self.imm(2, 128);
        let minimum = self.label();
        self.imm(3, 2);
        self.cmp(8, 3);
        self.branch(minimum, Branch::Cond(0));
        self.imm(2, 2048);
        self.imm(3, 3);
        self.cmp(8, 3);
        self.branch(minimum, Branch::Cond(0));
        self.imm(2, 65536);
        self.mark(minimum);
        self.cmp(0, 2);
        self.branch(invalid, Branch::Cond(3));
        self.imm(2, 0x10ffff);
        self.cmp(0, 2);
        self.branch(invalid, Branch::Cond(8));
        let scalar = self.label();
        self.imm(2, 0xd800);
        self.cmp(0, 2);
        self.branch(scalar, Branch::Cond(3));
        self.imm(2, 0xdfff);
        self.cmp(0, 2);
        self.branch(invalid, Branch::Cond(9));
        self.mark(scalar);
        self.add(6, 6, 1);
        self.branch(head, Branch::Jump);
        self.mark(ascii);
        self.add_imm(6, 6, 1);
        self.branch(head, Branch::Jump);
        self.mark(invalid);
        self.imm(0, 92);
        self.epilogue();
        self.mark(done);
        self.imm(0, 0);
        self.epilogue();
    }
}
