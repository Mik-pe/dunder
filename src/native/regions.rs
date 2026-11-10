use super::{Branch, Code};
use crate::check::Expr;

impl Code<'_> {
    pub(super) fn scratch_alloc(
        &mut self,
        area: usize,
        cursor: usize,
        limit: usize,
        count: &Expr,
        storage: usize,
        element: crate::syntax::Type,
    ) {
        let stride = self
            .types
            .storage_bytes(element)
            .unwrap_or_else(|| self.types.scalar_size(element))
            .max(1);
        let alignment = self
            .types
            .record(element)
            .map(|r| if r.c_layout { r.alignment } else { 8 })
            .or_else(|| self.types.array(element).map(|a| a.elem_size()))
            .unwrap_or(stride);
        self.expr(count); // x0 = signed i32 element count
        self.cmp(0, 31);
        self.branch(self.bounds, Branch::Cond(11));
        self.load_reg(cursor, 1);
        if alignment > 1 {
            self.add_imm(1, 1, (alignment - 1) as u32);
            self.imm(2, !((alignment - 1) as u64));
            self.and(1, 1, 2);
        }
        self.mov(2, 0);
        if !stride.is_power_of_two() {
            self.imm(3, stride as u64);
            self.emit(0x9b037c42); // mul x2,x2,x3
        } else if stride > 1 {
            self.lsl(2, 2, stride.trailing_zeros());
        }
        self.add(3, 1, 2);
        self.load_reg(limit, 4);
        self.cmp(3, 4);
        self.branch(self.bounds, Branch::Cond(8));
        self.store(cursor, 3);
        self.load_reg(area, 4);
        self.ldr(4, 4);
        self.add(4, 4, 1);
        self.local_addr(storage);
        self.str(4, 9);
        self.add_imm(9, 9, 8);
        self.str(0, 9);
        self.add_imm(9, 9, 8);
        self.str(0, 9);

        // The backing block can have come from a previous invocation. Clear
        // only requested bytes, leaving alignment gaps and unused capacity untouched.
        let clear = self.label();
        let done = self.label();
        self.branch(done, Branch::Zero(2));
        self.mark(clear);
        let zero_width = 1 << stride.trailing_zeros().min(3);
        let word = match zero_width {
            1 => crate::syntax::Type::U8,
            2 => crate::syntax::Type::U16,
            4 => crate::syntax::Type::U32,
            _ => crate::syntax::Type::U64,
        };
        self.typed_store_reg(word, 4, 31);
        self.add_imm(4, 4, zero_width);
        self.sub_imm(2, 2, zero_width);
        self.branch(clear, Branch::Nonzero(2));
        self.mark(done);
        self.local_addr(storage);
        self.mov(0, 9);
    }
}
