use super::{Branch, Code};
use crate::{
    check::{EnumArm, Expr},
    syntax::Type,
};
impl Code<'_> {
    pub(super) fn enum_value(
        &mut self,
        ty: Type,
        tag: i32,
        fields: &[(usize, Expr)],
        storage: Option<usize>,
    ) {
        let Some(storage) = storage else {
            self.imm(0, tag as i64 as u64);
            return;
        };
        self.local_addr(storage);
        self.imm(11, self.types.storage_bytes(ty).unwrap() as u64);
        let zero = self.label();
        self.mark(zero);
        self.strb(31, 9);
        self.add_imm(9, 9, 1);
        self.sub_imm(11, 11, 1);
        self.branch(zero, Branch::Nonzero(11));
        self.imm(0, tag as i64 as u64);
        self.local_addr(storage);
        self.typed_store(Type::I32, 9);
        for (offset, value) in fields {
            self.expr(value);
            self.local_addr(storage);
            self.imm(11, *offset as u64);
            self.add(9, 9, 11);
            if let Some(bytes) = self.types.storage_bytes(value.ty) {
                self.mov(10, 0);
                self.mov(11, 9);
                self.imm(12, bytes as u64);
                self.copy_bytes(11, 10, 12, 13);
            } else {
                self.typed_store(value.ty, 9);
            }
        }
        self.local_addr(storage);
        self.mov(0, 9);
    }
    pub(super) fn enum_match(
        &mut self,
        source: &Expr,
        slot: usize,
        storage: Option<usize>,
        arms: &[EnumArm],
    ) {
        self.expr(source);
        if let Some(storage) = storage {
            self.copy_value(storage, source.ty);
        }
        self.store(slot, 0);
        let done = self.label();
        let labels: Vec<_> = (0..arms.len()).map(|_| self.label()).collect();
        self.load(slot);
        if storage.is_some() {
            self.typed_load(Type::I32);
        }
        for (arm, label) in arms.iter().zip(&labels) {
            if let Some(tag) = arm.tag {
                self.imm(1, tag as i64 as u64);
                self.cmp(0, 1);
                self.branch(*label, Branch::Cond(0));
            } else {
                self.branch(*label, Branch::Jump);
            }
        }
        // The checker proves exhaustiveness; keep a defined failure for corrupted tags.
        self.branch(self.invalid_enum, Branch::Jump);
        for (arm, label) in arms.iter().zip(labels) {
            self.mark(label);
            for binding in &arm.bindings {
                self.load(slot);
                self.imm(1, binding.offset as u64);
                self.add(0, 0, 1);
                if let Some(storage) = binding.storage {
                    self.copy_value(storage, binding.ty);
                } else {
                    self.typed_load(binding.ty);
                }
                self.store(binding.slot, 0);
            }
            self.stmts(&arm.body);
            self.branch(done, Branch::Jump);
        }
        self.mark(done);
    }
}
