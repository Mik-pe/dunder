//! Typed dynamic storage. Descriptors are values; borrows never escape calls.
use super::{Branch, Code, Label};
use crate::{
    check::{DynamicOp, Expr},
    syntax::Type,
};
const ARENA: u64 = 512 * 1024 * 1024;
// The payload budget is unchanged; one bookkeeping page holds segregated
// free-list heads and allows a single full-budget allocation plus its header.
const MAPPING: u64 = ARENA + 4096;
const BIN_BYTES: u32 = 512;
const HEADER: u32 = 16;
#[derive(Default)]
pub(super) struct Runtime {
    alloc: Label,
    pub release: Label,
    pub grow: Label,
    clone: Label,
    exhausted: Label,
}
impl Runtime {
    pub fn new(c: &mut Code<'_>) -> Self {
        Self {
            alloc: c.label(),
            release: c.label(),
            grow: c.label(),
            clone: c.label(),
            exhausted: c.label(),
        }
    }
}
impl Code<'_> {
    pub(super) fn integer_expr(
        &mut self,
        op: crate::check::IntOp,
        base: &Expr,
        arg: Option<&Expr>,
    ) {
        use crate::check::IntOp;
        self.expr(base);
        self.push(0);
        if let Some(arg) = arg {
            self.expr(arg);
        }
        self.pop(1);
        let wide = base.ty == Type::I64;
        if matches!(op, IntOp::Shl | IntOp::Shr) {
            self.imm(2, if wide { 64 } else { 32 });
            self.cmp(0, 2);
            self.branch(self.bounds, Branch::Cond(2));
        }
        let word = match op {
            IntOp::And => 0x8a000020,
            IntOp::Or => 0xaa000020,
            IntOp::Xor => 0xca000020,
            IntOp::Not => 0xaa2103e0,
            IntOp::Shl => 0x9ac02020,
            IntOp::Shr => 0x9ac02420,
            IntOp::WrappingAdd => 0x8b000020,
            IntOp::WrappingSub => 0xcb000020,
            IntOp::WrappingMul => 0x9b007c20,
        };
        self.emit(if wide { word } else { word & 0x7fffffff });
        if !wide {
            self.emit(0x93407c00);
        }
    }
    fn needs_clone(&self, ty: Type) -> bool {
        matches!(ty, Type::String | Type::Bytes | Type::Runes)
            || self.types.dynamic_elem(ty).is_some()
            || self
                .types
                .array(ty)
                .is_some_and(|a| self.needs_clone(a.elem))
            || self
                .types
                .record(ty)
                .is_some_and(|r| r.fields.iter().any(|f| self.needs_clone(f.ty)))
            || self.types.enumeration(ty).is_some_and(|e| {
                e.variants
                    .iter()
                    .any(|v| v.fields.iter().any(|f| self.needs_clone(f.ty)))
            })
    }
    pub(super) fn needs_drop(&self, ty: Type) -> bool {
        self.needs_clone(ty)
    }
    // x0 addresses an owned value. Drop only its active owned buffers and
    // immutable-string references, retaining the address in x0.
    pub(super) fn drop_value(&mut self, ty: Type) {
        if !self.needs_drop(ty) {
            return;
        }
        if matches!(ty, Type::String | Type::Bytes | Type::Runes) {
            self.push(0);
            self.ldr(0, 0);
            self.branch(self.strings.release, Branch::Call);
            self.pop(0);
            self.str(31, 0);
            return;
        }
        if let Some(elem) = self.types.dynamic_elem(ty) {
            if elem == Type::String {
                self.string_elements(true, 0, false);
            }
            self.push(0);
            self.ldr(0, 0);
            self.branch(self.collections.release, Branch::Call);
            self.pop(0);
            self.str(31, 0);
            self.add_imm(1, 0, 8);
            self.str(31, 1);
            self.add_imm(1, 0, 16);
            self.str(31, 1);
            return;
        }
        if let Some(array) = self.types.array(ty) {
            if array.elem == Type::String {
                self.string_elements(false, array.len, false);
            }
            return;
        }
        if let Some(record) = self.types.record(ty).cloned() {
            for field in record.fields {
                self.drop_field(field.ty, field.offset);
            }
            return;
        }
        if let Some(enumeration) = self.types.enumeration(ty).cloned() {
            let done = self.label();
            for variant in enumeration.variants {
                let next = self.label();
                self.push(0);
                self.typed_load(Type::I32);
                self.imm(1, variant.tag as i64 as u64);
                self.cmp(0, 1);
                self.pop(0);
                self.branch(next, Branch::Cond(1));
                for field in variant.fields {
                    self.drop_field(field.ty, field.offset);
                }
                self.branch(done, Branch::Jump);
                self.mark(next);
            }
            self.mark(done);
        }
    }
    fn drop_field(&mut self, ty: Type, offset: usize) {
        if self.needs_drop(ty) {
            self.push(0);
            self.imm(1, offset as u64);
            self.add(0, 0, 1);
            self.drop_value(ty);
            self.pop(0);
        }
    }
    // x0: source, x1: destination; returns destination in x0.
    pub(super) fn clone_into(&mut self, ty: Type) {
        self.push(1);
        self.mov(10, 0);
        self.mov(11, 1);
        self.imm(12, self.types.storage_bytes(ty).unwrap() as u64);
        self.copy_bytes(11, 10, 12, 13);
        self.pop(0);
        self.clone_fields(ty);
    }
    fn clone_fields(&mut self, ty: Type) {
        if !self.needs_clone(ty) {
            return;
        }
        if matches!(ty, Type::String | Type::Bytes | Type::Runes) {
            self.push(0);
            self.ldr(0, 0);
            self.branch(self.strings.retain, Branch::Call);
            self.pop(0);
            return;
        }
        if let Some(elem) = self.types.dynamic_elem(ty) {
            self.imm(1, self.types.scalar_size(elem) as u64);
            self.branch(self.collections.clone, Branch::Call);
            if elem == Type::String {
                self.string_elements(true, 0, true);
            }
            return;
        }
        if let Some(array) = self.types.array(ty) {
            if array.elem == Type::String {
                self.string_elements(false, array.len, true);
            }
            return;
        }
        if let Some(r) = self.types.record(ty).cloned() {
            for f in r.fields {
                self.clone_field(f.ty, f.offset);
            }
            return;
        }
        if let Some(e) = self.types.enumeration(ty).cloned() {
            let done = self.label();
            for v in e.variants {
                let next = self.label();
                self.push(0);
                self.typed_load(Type::I32);
                self.imm(1, v.tag as i64 as u64);
                self.cmp(0, 1);
                self.pop(0);
                self.branch(next, Branch::Cond(1));
                for f in v.fields {
                    self.clone_field(f.ty, f.offset);
                }
                self.branch(done, Branch::Jump);
                self.mark(next);
            }
            self.mark(done);
        }
    }
    fn clone_field(&mut self, ty: Type, offset: usize) {
        if self.needs_clone(ty) {
            self.push(0);
            self.imm(1, offset as u64);
            self.add(0, 0, 1);
            self.clone_fields(ty);
            self.pop(0);
        }
    }
    // Retain or release the live strings in an array, preserving its address.
    // Copies own independent buffers but share immutable string allocations.
    fn string_elements(&mut self, dynamic: bool, length: usize, retain: bool) {
        self.push(0);
        if dynamic {
            self.add_imm(11, 0, 8);
            self.ldr(11, 11);
            self.ldr(10, 0);
        } else {
            self.mov(10, 0);
            self.imm(11, length as u64);
        }
        let next = self.label();
        let done = self.label();
        self.mark(next);
        self.branch(done, Branch::Zero(11));
        self.push(10);
        self.push(11);
        self.ldr(0, 10);
        self.branch(
            if retain {
                self.strings.retain
            } else {
                self.strings.release
            },
            Branch::Call,
        );
        self.pop(11);
        self.pop(10);
        if !retain {
            self.str(31, 10);
        }
        self.add_imm(10, 10, 8);
        self.sub_imm(11, 11, 1);
        self.branch(next, Branch::Jump);
        self.mark(done);
        self.pop(0);
    }
    pub(super) fn dynamic_value(&mut self, ty: Type, values: &[Expr], storage: usize) {
        let elem = self.types.dynamic_elem(ty).unwrap();
        let size = self.types.scalar_size(elem);
        self.local_addr(storage);
        self.str(31, 9);
        self.add_imm(9, 9, 8);
        self.str(31, 9);
        self.add_imm(9, 9, 8);
        self.str(31, 9);
        if !values.is_empty() {
            self.local_addr(storage);
            self.mov(0, 9);
            self.imm(1, values.len() as u64);
            self.imm(2, size as u64);
            self.branch(self.collections.grow, Branch::Call);
            for (i, v) in values.iter().enumerate() {
                self.expr(v);
                if elem == Type::String {
                    self.branch(self.strings.retain, Branch::Call);
                }
                self.local_addr(storage);
                self.ldr(9, 9);
                self.imm(11, (i * size) as u64);
                self.add(9, 9, 11);
                self.typed_store(elem, 9);
            }
            self.local_addr(storage);
            self.add_imm(9, 9, 8);
            self.imm(0, values.len() as u64);
            self.str(0, 9);
        }
        self.local_addr(storage);
        self.mov(0, 9);
    }
    pub(super) fn dynamic_expr(
        &mut self,
        op: DynamicOp,
        base: &Expr,
        arg: Option<&Expr>,
        ty: Type,
    ) {
        let elem = self.types.dynamic_elem(base.ty).unwrap();
        let size = self.types.scalar_size(elem);
        self.expr(base);
        match op {
            DynamicOp::Len | DynamicOp::Capacity => {
                self.add_imm(0, 0, if matches!(op, DynamicOp::Len) { 8 } else { 16 });
                self.ldr(0, 0);
            }
            DynamicOp::Clear => {
                if elem == Type::String {
                    self.string_elements(true, 0, false);
                }
                self.add_imm(0, 0, 8);
                self.str(31, 0);
            }
            DynamicOp::Reserve => {
                self.push(0);
                self.expr(arg.unwrap());
                self.mov(1, 0);
                self.pop(0);
                self.imm(2, size as u64);
                self.branch(self.collections.grow, Branch::Call);
            }
            DynamicOp::Push => {
                self.push(0);
                self.expr(arg.unwrap());
                if elem == Type::String {
                    self.branch(self.strings.retain, Branch::Call);
                }
                self.push(0);
                self.emit(0xf9400be0);
                self.add_imm(1, 0, 8);
                self.ldr(1, 1);
                self.add_imm(1, 1, 1);
                self.imm(2, size as u64);
                self.branch(self.collections.grow, Branch::Call);
                self.pop(1);
                self.pop(0);
                self.add_imm(3, 0, 8);
                self.ldr(2, 3);
                self.add_imm(4, 2, 1);
                self.str(4, 3);
                self.ldr(9, 0);
                self.imm(3, size as u64);
                self.emit(0x9b037c42);
                self.add(9, 9, 2);
                self.mov(0, 1);
                self.typed_store(elem, 9);
            }
            DynamicOp::Pop => {
                self.add_imm(1, 0, 8);
                self.ldr(2, 1);
                self.branch(self.bounds, Branch::Zero(2));
                self.sub_imm(2, 2, 1);
                self.str(2, 1);
                self.ldr(0, 0);
                self.imm(3, size as u64);
                self.emit(0x9b037c42);
                self.add(0, 0, 2);
                if elem == Type::String {
                    self.mov(9, 0);
                }
                self.typed_load(ty);
                if elem == Type::String {
                    // The result takes the array's existing reference.
                    self.str(31, 9);
                }
            }
        }
    }
    pub(super) fn collections_runtime(&mut self) {
        self.mark(self.collections.alloc);
        let mapped = self.label();
        self.branch(mapped, Branch::Nonzero(21));
        self.push(0);
        self.imm(0, 0);
        self.imm(1, MAPPING);
        self.imm(2, 3);
        self.imm(
            3,
            if self.target == crate::target::Target::Arm64Macos {
                0x1002
            } else {
                0x22
            },
        );
        self.imm(4, u64::MAX);
        self.imm(5, 0);
        self.syscall(197);
        self.branch(self.collections.exhausted, Branch::Cond(2));
        self.mov(21, 0);
        self.imm(1, MAPPING);
        self.add(22, 21, 1);
        self.add_imm(21, 21, BIN_BYTES);
        self.pop(0);
        self.mark(mapped);
        self.imm(1, ARENA);
        self.cmp(0, 1);
        self.branch(self.collections.exhausted, Branch::Cond(8));
        self.imm(1, 16); // Payload size and zero-based size class.
        self.imm(2, 0);
        let classify = self.label();
        let classified = self.label();
        self.mark(classify);
        self.cmp(1, 0);
        self.branch(classified, Branch::Cond(2));
        self.lsl(1, 1, 1);
        self.add_imm(2, 2, 1);
        self.branch(classify, Branch::Jump);
        self.mark(classified);
        self.imm(3, MAPPING);
        self.sub(3, 22, 3);
        self.lsl(4, 2, 3);
        self.add(4, 3, 4);
        self.ldr(5, 4);
        let fresh = self.label();
        let allocated = self.label();
        self.branch(fresh, Branch::Zero(5));
        self.add_imm(3, 5, 8);
        self.ldr(3, 3);
        self.str(3, 4);
        self.branch(allocated, Branch::Jump);
        self.mark(fresh);
        self.add(3, 21, 1);
        self.add_imm(3, 3, HEADER);
        self.cmp(3, 22);
        self.branch(self.collections.exhausted, Branch::Cond(8));
        self.mov(5, 21);
        self.mov(21, 3);
        self.mark(allocated);
        self.add_imm(2, 2, 1); // Zero marks a free block; catches double release.
        self.str(2, 5);
        self.add_imm(0, 5, HEADER);
        self.ret();

        self.mark(self.collections.release);
        let released = self.label();
        self.branch(released, Branch::Zero(0));
        self.sub_imm(0, 0, HEADER);
        self.ldr(1, 0);
        self.branch(self.collections.exhausted, Branch::Zero(1));
        self.imm(2, 26);
        self.cmp(1, 2);
        self.branch(self.collections.exhausted, Branch::Cond(8));
        self.str(31, 0);
        self.sub_imm(1, 1, 1);
        self.lsl(1, 1, 3);
        self.imm(2, MAPPING);
        self.sub(2, 22, 2);
        self.add(1, 2, 1);
        self.ldr(2, 1);
        self.add_imm(3, 0, 8);
        self.str(2, 3);
        self.str(0, 1);
        self.mark(released);
        self.ret();

        self.mark(self.collections.grow); // descriptor x0, minimum x1, element size x2
        self.prologue(64);
        self.store(0, 0);
        self.store(1, 1);
        self.store(2, 2);
        self.imm(3, ARENA);
        self.emit(0x9ac20863);
        self.cmp(1, 3);
        self.branch(self.collections.exhausted, Branch::Cond(8));
        self.add_imm(3, 0, 16);
        self.ldr(3, 3);
        self.cmp(1, 3);
        let done = self.label();
        self.branch(done, Branch::Cond(9));
        let capacity = self.label();
        let enough = self.label();
        self.branch(capacity, Branch::Nonzero(3));
        self.imm(3, 8);
        self.mark(capacity);
        self.cmp(3, 1);
        self.branch(enough, Branch::Cond(2));
        self.lsl(3, 3, 1);
        self.branch(capacity, Branch::Jump);
        self.mark(enough);
        self.store(3, 3);
        self.emit(0x9b027c60);
        self.branch(self.collections.alloc, Branch::Call);
        self.store(4, 0);
        self.load(0);
        self.ldr(10, 0);
        self.add_imm(1, 0, 8);
        self.ldr(12, 1);
        self.load(2);
        self.emit(0x9b007d8c);
        self.load(4);
        self.mov(11, 0);
        self.copy_bytes(11, 10, 12, 13);
        self.load(0);
        self.ldr(0, 0);
        self.branch(self.collections.release, Branch::Call);
        self.load(0);
        self.mov(10, 0);
        self.load(4);
        self.str(0, 10);
        self.add_imm(10, 10, 16);
        self.load(3);
        self.str(0, 10);
        self.mark(done);
        self.load(0);
        self.epilogue();

        self.mark(self.collections.clone); // descriptor x0, element size x1
        self.prologue(64);
        self.store(0, 0);
        self.store(1, 1);
        self.ldr(2, 0);
        self.store(2, 2); // The old pointer belongs to the source, never release it.
        self.add_imm(2, 0, 16);
        self.str(31, 2);
        self.add_imm(2, 0, 8);
        self.ldr(2, 2);
        self.store(3, 2);
        let nonempty = self.label();
        self.branch(nonempty, Branch::Nonzero(2));
        self.str(31, 0);
        self.epilogue();
        self.mark(nonempty);
        self.emit(0x9b017c40); // x0 = length * element size.
        self.branch(self.collections.alloc, Branch::Call);
        self.store(4, 0);
        self.mov(11, 0);
        self.load(2);
        self.mov(10, 0);
        self.load(3);
        self.mov(12, 0);
        self.load(1);
        self.emit(0x9b007d8c); // x12 *= element size.
        self.copy_bytes(11, 10, 12, 13);
        self.load(0);
        self.mov(10, 0);
        self.load(4);
        self.str(0, 10);
        self.add_imm(10, 10, 16);
        self.load(3);
        self.str(0, 10);
        self.load(0);
        self.epilogue();
        self.mark(self.collections.exhausted);
        self.imm(0, 109);
        self.syscall(1);
        self.emit(0xd4200000);
    }
}
