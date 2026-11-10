//! Local, typed instruction selection. Every rewrite preserves source evaluation
//! order and runtime traps; no floating-point reassociation or unchecked arithmetic.
use super::{Branch, Code, Label};
use crate::{
    check::{Expr, ExprKind},
    syntax::Type,
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
    fn simple(e: &Expr) -> bool {
        matches!(
            e.kind,
            ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Float(_) | ExprKind::Local(_)
        )
    }
    // Only x9 may be used as address scratch; x0/x1 are independent destinations.
    fn simple_into(&mut self, e: &Expr, reg: u32) {
        match e.kind {
            ExprKind::Number(n) => self.imm(reg, n as u64),
            ExprKind::Bool(b) => self.imm(reg, u64::from(b)),
            ExprKind::Float(bits) => self.imm(reg, bits),
            ExprKind::Local(slot) => {
                self.load_reg(slot, reg);
                // Mutable borrows store narrow values into eight-byte local
                // slots. Match ordinary Local lowering's sign extension.
                if matches!(e.ty, Type::I32 | Type::Rune) || self.types.scalar_enum(e.ty) {
                    self.emit(0x93407c00 | reg << 5 | reg);
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
        if !e.ty.integer() {
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
