//! Typed dynamic storage. Descriptors are values; borrows never escape calls.
use super::{Branch, Code, Label};
use crate::{
    check::{DynamicOp, Expr},
    syntax::Type,
};
const ARENA: u64 = 512 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Runtime {
    alloc: Label,
    pub grow: Label,
    clone: Label,
    exhausted: Label,
}
impl Runtime {
    pub fn new(c: &mut Code<'_>) -> Self {
        Self {
            alloc: c.label(),
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
        self.types.dynamic_elem(ty).is_some()
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
        if let Some(elem) = self.types.dynamic_elem(ty) {
            self.imm(1, self.types.scalar_size(elem) as u64);
            self.branch(self.collections.clone, Branch::Call);
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
                self.typed_load(ty);
            }
        }
    }
    pub(super) fn collections_runtime(&mut self) {
        self.mark(self.collections.alloc);
        let mapped = self.label();
        self.branch(mapped, Branch::Nonzero(21));
        self.push(0);
        self.imm(0, 0);
        self.imm(1, ARENA);
        self.imm(2, 3);
        self.imm(3, 0x1002);
        self.imm(4, u64::MAX);
        self.imm(5, 0);
        self.imm(16, 0x20000c5);
        self.emit(0xd4001001);
        self.branch(self.collections.exhausted, Branch::Cond(2));
        self.mov(21, 0);
        self.imm(1, ARENA);
        self.add(22, 21, 1);
        self.pop(0);
        self.mark(mapped);
        self.add_imm(1, 0, 7);
        self.lsr(1, 1, 3);
        self.lsl(1, 1, 3);
        self.add(2, 21, 1);
        self.cmp(2, 22);
        self.branch(self.collections.exhausted, Branch::Cond(8));
        self.mov(0, 21);
        self.mov(21, 2);
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
        self.add_imm(2, 0, 16);
        self.str(31, 2);
        self.add_imm(2, 0, 8);
        self.ldr(2, 2);
        let nonempty = self.label();
        self.branch(nonempty, Branch::Nonzero(2));
        self.str(31, 0);
        self.ret();
        self.mark(nonempty);
        self.mov(3, 1);
        self.mov(1, 2);
        self.mov(2, 3);
        self.branch(self.collections.grow, Branch::Jump);
        self.mark(self.collections.exhausted);
        self.imm(0, 109);
        self.imm(16, 0x2000001);
        self.emit(0xd4001001);
        self.emit(0xd4200000);
    }
}
