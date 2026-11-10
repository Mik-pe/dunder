//! Local, typed instruction selection. Every rewrite preserves source evaluation
//! order and runtime traps; no floating-point reassociation or unchecked arithmetic.
use super::{Branch, Code, Label};
use crate::{
    check::{DynamicOp, Expr, ExprKind, Function, Stmt},
    syntax::{Type, Types},
    target::Target,
};

/// Evaluate only pure scalar constants whose checked operations succeed. Failed
/// arithmetic remains generated code so its trap occurs at the original runtime
/// position, after any preceding effects and only on the executed path.
pub(super) fn constant(e: &Expr) -> Option<i64> {
    if !matches!(e.ty, Type::I32 | Type::I64 | Type::Bool | Type::Rune) {
        return None;
    }
    let value = match &e.kind {
        ExprKind::Number(n) => *n,
        ExprKind::Bool(b) => i64::from(*b),
        ExprKind::Cast(x) if x.ty.integer() && e.ty.integer() => constant(x)?,
        ExprKind::Unary("-", x) if e.ty.integer() => constant(x)?.checked_neg()?,
        ExprKind::Unary("!", x) => i64::from(constant(x)? == 0),
        ExprKind::Binary(op, a, b) => {
            let a = constant(a)?;
            // Skipped right operands are not evaluated by the source program.
            if *op == "&&" && a == 0 {
                return Some(0);
            }
            if *op == "||" && a != 0 {
                return Some(1);
            }
            let b = constant(b)?;
            // Both division and remainder trap for the signed MIN / -1 pair,
            // including i32 where evaluating in Rust's i64 would otherwise fit.
            if e.ty == Type::I32 && matches!(*op, "/" | "%") && a == i64::from(i32::MIN) && b == -1
            {
                return None;
            }
            match *op {
                "+" => a.checked_add(b)?,
                "-" => a.checked_sub(b)?,
                "*" => a.checked_mul(b)?,
                "/" => a.checked_div(b)?,
                "%" => a.checked_rem(b)?,
                "==" => i64::from(a == b),
                "!=" => i64::from(a != b),
                "<" => i64::from(a < b),
                "<=" => i64::from(a <= b),
                ">" => i64::from(a > b),
                ">=" => i64::from(a >= b),
                "&&" => i64::from(b != 0),
                "||" => i64::from(b != 0),
                _ => return None,
            }
        }
        _ => return None,
    };
    if e.ty == Type::I32 && i32::try_from(value).is_err() {
        return None;
    }
    Some(value)
}

impl Code<'_> {
    pub(super) fn simple(e: &Expr) -> bool {
        matches!(
            e.kind,
            ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Float(_) | ExprKind::Local(_)
        )
    }
    // Only x9 may be used as address scratch; x0/x1 are independent destinations.
    pub(super) fn simple_into(&mut self, e: &Expr, reg: u32) {
        match e.kind {
            ExprKind::Number(n) => self.imm(reg, n as u64),
            ExprKind::Bool(b) => self.imm(reg, u64::from(b)),
            ExprKind::Float(bits) => self.imm(reg, bits),
            ExprKind::Local(slot) => {
                self.load_reg(slot + self.locals.inline_offset, reg);
                // Mutable borrows store narrow values into eight-byte local
                // slots. Match ordinary Local lowering's sign extension.
                if matches!(e.ty, Type::I32 | Type::Rune) || self.types.scalar_enum(e.ty) {
                    self.emit(0x93407c00 | reg << 5 | reg);
                } else if e.ty.unsigned() {
                    self.zero_extend(reg, self.unsigned_bits(e.ty));
                }
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn operands(&mut self, left: &Expr, right: &Expr) {
        if Self::simple(right) {
            if Self::simple(left) {
                self.simple_into(left, 1);
            } else {
                self.expr(left);
                self.mov(1, 0);
            }
            self.simple_into(right, 0);
        } else if Self::simple(left) && read_only(right, 6) {
            // A simple local/literal cannot trap. A pure right operand cannot
            // change it, so the left register can be loaded after evaluation.
            self.expr(right);
            self.simple_into(left, 1);
        } else {
            self.expr(left);
            self.push(0);
            self.expr(right);
            self.pop(1);
        }
    }
    pub(super) fn optimized_expr(&mut self, e: &Expr) -> bool {
        if let Some(value) = constant(e) {
            self.imm(0, value as u64);
            return true;
        }
        let ExprKind::Binary(op, left, right) = &e.kind else {
            return false;
        };
        if !matches!(e.ty, Type::I32 | Type::I64) {
            return false;
        }
        let Some(value) = constant(right) else {
            return false;
        };
        if (matches!(*op, "+" | "-") && value == 0) || (matches!(*op, "*" | "/") && value == 1) {
            self.expr(left);
            return true;
        }
        if *op == "*" && value == 0 {
            self.expr(left); // Calls, side effects and traps must still happen.
            self.imm(0, 0);
            return true;
        }
        if matches!(*op, "+" | "-") && value.unsigned_abs() <= 4095 {
            self.expr(left);
            let subtract = (*op == "-") != (value < 0);
            let width = if e.ty == Type::I64 { 0x80000000 } else { 0 };
            self.emit(
                0x31000000
                    | width
                    | if subtract { 0x40000000 } else { 0 }
                    | ((value.unsigned_abs() as u32) << 10),
            );
            self.branch(self.overflow, Branch::Cond(6));
            if e.ty == Type::I32 {
                self.emit(0x93407c00);
            }
            return true;
        }
        false
    }
    pub(super) fn condition(&mut self, e: &Expr, target: Label, when: bool) {
        if let Some(value) = constant(e) {
            if (value != 0) == when {
                self.branch(target, Branch::Jump);
            }
            return;
        }
        match &e.kind {
            ExprKind::Unary("!", value) => self.condition(value, target, !when),
            ExprKind::Binary(op, left, right) if matches!(*op, "&&" | "||") => {
                if (*op == "&&") != when {
                    self.condition(left, target, when);
                    self.condition(right, target, when);
                } else {
                    let skipped = self.label();
                    self.condition(left, skipped, !when);
                    self.condition(right, target, when);
                    self.mark(skipped);
                }
            }
            ExprKind::Binary(op, left, right)
                if matches!(*op, "==" | "!=" | "<" | "<=" | ">" | ">=")
                    && (matches!(left.ty, Type::I32 | Type::I64 | Type::Rune | Type::Bool)
                        || self.types.scalar_enum(left.ty)) =>
            {
                self.operands(left, right);
                self.cmp(1, 0);
                let condition = match *op {
                    "==" => 0,
                    "!=" => 1,
                    "<" => 11,
                    "<=" => 13,
                    ">" => 12,
                    ">=" => 10,
                    _ => unreachable!(),
                };
                self.branch(target, Branch::Cond(condition ^ u32::from(!when)));
            }
            _ => {
                self.expr(e);
                self.branch(
                    target,
                    if when {
                        Branch::Nonzero(0)
                    } else {
                        Branch::Zero(0)
                    },
                );
            }
        }
    }
}

fn read_only(e: &Expr, depth: usize) -> bool {
    if depth == 0 { return false; }
    match &e.kind {
        ExprKind::Local(_) | ExprKind::ReferenceAddress(_) | ExprKind::Number(_) |
        ExprKind::Float(_) | ExprKind::Bool(_) => true,
        ExprKind::Deref(a) | ExprKind::Field(a, _) | ExprKind::Offset(a, _) |
        ExprKind::Cast(a) | ExprKind::Unary(_, a) |
        ExprKind::DynamicOp(DynamicOp::Len | DynamicOp::Capacity, a, None) => read_only(a, depth - 1),
        ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) =>
            read_only(a, depth - 1) && read_only(b, depth - 1) && a.ty != Type::String,
        _ => false,
    }
}

fn reference_root(expr: &Expr) -> Option<usize> {
    match &expr.kind {
        ExprKind::ReferenceAddress(slot) => Some(*slot),
        ExprKind::Deref(value) => match value.kind {
            ExprKind::ReferenceAddress(slot) => Some(slot),
            _ => None,
        },
        _ => None,
    }
}

const INLINE_PARAMS: usize = 4;

fn scalar(ty: Type) -> bool {
    ty.numeric() || matches!(ty, Type::Bool | Type::Rune)
}

/// Leaf bodies have no frame-local state and read only their scalar parameters.
/// Calls, memory accesses and allocations are deliberately outside this budget.
/// Checked arithmetic still goes through the ordinary trapping emitter.
pub(crate) fn inline_body(f: &Function) -> Option<&Expr> {
    let [Stmt::Return(Some(body))] = &f.body[..] else {
        return None;
    };
    if f.params.len() > INLINE_PARAMS || !f.params.iter().copied().all(scalar) {
        return None;
    }
    fn cost(e: &Expr, remaining: &mut usize) -> bool {
        if *remaining == 0 || !scalar(e.ty) {
            return false;
        }
        *remaining -= 1;
        match &e.kind {
            ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Float(_) | ExprKind::Local(_) => {
                true
            }
            ExprKind::Cast(a) | ExprKind::Unary(_, a) | ExprKind::FloatOp(_, a) => {
                cost(a, remaining)
            }
            ExprKind::IntOp(_, a, b) => {
                cost(a, remaining) && b.as_deref().is_none_or(|b| cost(b, remaining))
            }
            ExprKind::Binary(_, a, b) => cost(a, remaining) && cost(b, remaining),
            _ => false,
        }
    }
    if let ExprKind::Record(fields, _) = &body.kind {
        let mut remaining = 9;
        fields.iter().all(|(_, e)| cost(e, &mut remaining)).then_some(body)
    } else {
        cost(body, &mut 10).then_some(body)
    }
}

#[derive(Default)]
pub(super) struct LocalPlan {
    homes: Vec<u8>,
    pub(super) array_parameter: Option<usize>,
    move_reads: Vec<u8>,
    count: u32,
    scratch: usize,
    pub(super) inline_slots: usize,
    inline_offset: usize,
}

impl LocalPlan {
    pub(super) fn new(f: &Function, leaves: &[Option<&Expr>], target: Target, types: &Types) -> Self {
        let mut weights = vec![0u32; f.slots];
        let mut inline_slots = 0;
        // A sole, non-repeating read can transfer an owner to another local.
        // Any previous address-taking, additional read or enclosing loop
        // rules it out. Return expressions have their own terminal-use rule.
        let mut move_reads = if f.owned_storage.is_empty() {
            Vec::new()
        } else {
            vec![0u8; f.slots]
        };
        // A scalar-only leaf with a single borrowed array parameter can keep
        // the buffer and length in caller-saved registers throughout. Count all
        // uses of the reference, allowing only indexing and length queries.
        // Passing the reference elsewhere, replacing its descriptor, pointer
        // operations, calls and resizing all disqualify the cache.
        let mut array_parameter = if f.owned_storage.is_empty() && target != Target::X86_64Linux {
            f.params.iter().enumerate().find_map(|(i, &ty)| {
                types.referenced(ty)
                    .filter(|(t, _)| types.dynamic_elem(*t).is_some_and(scalar))
                    .filter(|_| f.params.iter().enumerate().all(|(j, &t)| i == j || scalar(t)))
                    .map(|_| f.param_slots[i])
            })
        } else { None };
        let mut reference_uses = 0;
        let mut indexed_uses = 0;
        let mut repeated_index = false;
        super::reachable::visit_body(&f.body, false, &mut |e, repeated| {
            if let Some(slot) = array_parameter {
                match &e.kind {
                    ExprKind::ReferenceAddress(other) if *other == slot => reference_uses += 1,
                    ExprKind::Index(base, _) | ExprKind::ElementAddress(base, _)
                    | ExprKind::DynamicOp(DynamicOp::Len, base, None)
                        if reference_root(base) == Some(slot) => {
                        indexed_uses += 1;
                        repeated_index |= repeated;
                    }
                    ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Float(_) |
                    ExprKind::Local(_) | ExprKind::Deref(_) | ExprKind::Cast(_) |
                    ExprKind::Unary(_, _) | ExprKind::Range(_, _) | ExprKind::IntOp(_, _, _) |
                    ExprKind::Binary(_, _, _) => {}
                    _ => array_parameter = None,
                }
            }
            if !move_reads.is_empty() {
                match e.kind {
                    ExprKind::Local(slot) => {
                        move_reads[slot] = if repeated {
                            2
                        } else {
                            move_reads[slot].saturating_add(1)
                        };
                    }
                    ExprKind::LocalAddress(slot) | ExprKind::ReferenceAddress(slot) => {
                        move_reads[slot] = 2
                    }
                    _ => {}
                }
            }
            match &e.kind {
                ExprKind::Local(slot) if repeated && scalar(e.ty) => {
                    weights[*slot] = weights[*slot].saturating_add(1);
                }
                ExprKind::LocalAddress(slot) => weights[*slot] = u32::MAX,
                ExprKind::Call(target, args, _) => {
                    if let ExprKind::Function(index) = target.kind {
                        if leaves[index].is_some() {
                            inline_slots = inline_slots.max(args.len());
                        }
                    }
                }
                _ => {}
            }
        });
        if !repeated_index || indexed_uses != reference_uses {
            array_parameter = None;
        }
        let mut homes = Vec::new();
        let mut count = 0;
        // Only rbx remains free after the fixed x86 register mapping.
        let limit = if target == Target::X86_64Linux { 1 } else { 4 };
        for reg in 25..25 + limit {
            let Some((slot, _)) = weights
                .iter()
                .enumerate()
                .filter(|(_, w)| **w > 0 && **w < u32::MAX)
                .max_by_key(|(_, w)| *w)
            else {
                break;
            };
            weights[slot] = 0;
            if homes.is_empty() {
                homes.resize(f.slots, 0);
            }
            homes[slot] = reg as u8;
            count += 1;
        }
        Self {
            homes,
            array_parameter,
            move_reads,
            count,
            scratch: f.slots,
            inline_slots,
            inline_offset: 0,
        }
    }
    pub(super) fn fixed_array(&self, expr: &Expr) -> bool {
        self.array_parameter.is_some_and(|slot| reference_root(expr) == Some(slot))
    }
    pub(super) fn registers(&self) -> std::ops::Range<u32> {
        25..25 + self.count
    }
    pub(super) fn register(&self, slot: usize) -> Option<u32> {
        self.homes
            .get(slot)
            .copied()
            .filter(|r| *r != 0)
            .map(u32::from)
    }
}

impl Code<'_> {
    /// The caller owns this result and must install or release it.
    pub(super) fn take_string(&mut self, e: &Expr, dst: usize) -> bool {
        let ExprKind::ManagedString(inner, _, fresh) = &e.kind else {
            return false;
        };
        if *fresh {
            self.expr(inner);
            return true;
        }
        if let ExprKind::Local(slot) = inner.kind {
            if slot != dst && self.locals.move_reads.get(slot) == Some(&1) {
                self.load(slot);
                self.store(slot, 31);
                return true;
            }
        }
        false
    }

    pub(super) fn can_move_buffer(&self, e: &Expr, dst: usize) -> bool {
        matches!(e.ty, Type::Dynamic(_))
            && match e.kind {
                ExprKind::Local(slot) => {
                    slot != dst && self.locals.move_reads.get(slot) == Some(&1)
                }
                ExprKind::Call(_, _, Some(_)) | ExprKind::Dynamic(_, _) => true,
                _ => false,
            }
    }

    pub(super) fn function_epilogue(&mut self) {
        for reg in self.locals.registers().rev() {
            self.pop(reg);
        }
        self.epilogue();
    }
    pub(super) fn expr_into(&mut self, e: &Expr, reg: u32) {
        if Self::simple(e) {
            self.simple_into(e, reg);
        } else {
            self.expr(e);
            self.mov(reg, 0);
        }
    }
    pub(super) fn record_into(&mut self, e: &Expr, storage: usize) -> bool {
        match &e.kind {
            ExprKind::Record(fields, _) if fields.iter().all(|(_, f)| scalar(f.ty) && Self::simple(f)) => {
                self.record_value(e.ty, fields, storage);
                true
            }
            ExprKind::Call(target, args, _) => {
                let ExprKind::Function(index) = target.kind else { return false };
                let Some(body) = self.leaves[index] else { return false };
                if matches!(body.kind, ExprKind::Record(..)) {
                    self.inline_call(body, args, Some(storage));
                    true
                } else { false }
            }
            _ => false
        }
    }

    pub(super) fn inline_call(&mut self, body: &Expr, args: &[Expr], storage: Option<usize>) {
        let scratch = self.locals.scratch;
        // Complex operands can call another inline leaf or mutate a local
        // captured by an earlier operand. Snapshot each one in source order.
        if args.iter().all(Self::simple) {
            for (i, arg) in args.iter().enumerate() {
                self.simple_into(arg, 0);
                self.store(scratch + i, 0);
            }
        } else if let Some((last, first)) = args.split_last() {
            for arg in first {
                self.expr(arg);
                self.push(0);
            }
            self.expr(last);
            self.store(scratch + first.len(), 0);
            for i in (0..first.len()).rev() {
                self.pop(0);
                self.store(scratch + i, 0);
            }
        }
        if let ExprKind::Record(fields, _) = &body.kind {
            // The callee's slot zero holds its implicit result pointer; its
            // first scalar parameter therefore starts at slot one.
            self.locals.inline_offset = scratch.checked_sub(1).expect("caller has a record result slot");
            self.record_value(body.ty, fields, storage.expect("record result destination"));
        } else {
            self.locals.inline_offset = scratch;
            self.expr(body);
        }
        self.locals.inline_offset = 0;
    }
    pub(super) fn return_expr(&mut self, e: &Expr) {
        if let ExprKind::ManagedString(inner, _, fresh) = &e.kind {
            // A fresh result or the final use of a local owns one reference
            // already. Remove the local's ownership before whole-frame cleanup.
            if *fresh {
                self.expr(inner);
                return;
            }
            if let ExprKind::Local(slot) = inner.kind {
                self.load(slot);
                self.store(slot, 31);
                return;
            }
        }
        self.expr(e);
        self.retain_string(e.ty);
    }
    /// Move the three-word dynamic descriptor from x0 to the distinct and
    /// empty result slot in x1. Both the element buffer and any contained
    /// string references transfer as a unit.
    pub(super) fn move_buffer(&mut self) {
        for offset in 0..3 {
            self.emit(0xf9400002 | offset << 10); // ldr x2,[x0,#offset*8]
            self.emit(0xf9000022 | offset << 10); // str x2,[x1,#offset*8]
            self.emit(0xf900001f | offset << 10); // str xzr,[x0,#offset*8]
        }
        self.mov(0, 1);
    }
}
