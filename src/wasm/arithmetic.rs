use super::*;
use crate::check::{FloatOp, IntOp};
impl Emitter<'_, '_> {
    pub(super) fn unsigned_limit(ty: Type) -> Option<u32> {
        match ty {
            Type::U8 => Some(255),
            Type::U16 => Some(65535),
            _ => None,
        }
    }
    fn checked_narrow(&mut self, ty: Type) {
        if let Some(limit) = Self::unsigned_limit(ty) {
            let r = self.save(I32);
            self.get(r);
            self.int(I32, limit.into());
            self.op(0x4b);
            self.trap_if();
            self.get(r);
        }
    }
    pub(super) fn cast(&mut self, from: Type, to: Type) -> Result<(), String> {
        let (a, b) = (val(from), val(to));
        if from == to {
            return Ok(());
        }
        if !from.numeric() || !to.numeric() {
            if a != b {
                return Err(format!(
                    "unsupported cast from {} to {}",
                    from.name(),
                    to.name()
                ));
            }
            return Ok(());
        }
        let bits = if to == Type::U8 {
            8
        } else if to == Type::U16 {
            16
        } else if b == I32 {
            32
        } else {
            64
        };
        if !from.float() && !to.float() {
            let saved = self.save(a);
            let src_bits = if from == Type::U8 {
                8
            } else if from == Type::U16 {
                16
            } else if a == I32 {
                32
            } else {
                64
            };
            if !from.unsigned() && (to.unsigned() || bits < src_bits) {
                let min = if to.unsigned() {
                    0
                } else {
                    -(1i64 << (bits - 1))
                };
                self.get(saved);
                self.int(a, min);
                self.op(if a == I32 { 0x48 } else { 0x53 });
                self.trap_if();
            }
            let max_bits = bits - usize::from(!to.unsigned());
            if max_bits < src_bits - usize::from(!from.unsigned()) {
                let max = u64::MAX >> (64 - max_bits);
                self.get(saved);
                self.int(a, max as i64);
                self.op(match (a, from.unsigned()) {
                    (I32, false) => 0x4a,
                    (I32, true) => 0x4b,
                    (I64, false) => 0x55,
                    _ => 0x56,
                });
                self.trap_if();
            }
            self.get(saved);
        }
        let op = match (a, b) {
            (I32, I64) => {
                if from.unsigned() {
                    0xad
                } else {
                    0xac
                }
            }
            (I64, I32) => 0xa7,
            (I32, F32) => 0xb2 + u8::from(from.unsigned()),
            (I64, F32) => 0xb4 + u8::from(from.unsigned()),
            (I32, F64) => 0xb7 + u8::from(from.unsigned()),
            (I64, F64) => 0xb9 + u8::from(from.unsigned()),
            (F32, I32) => 0xa8 + u8::from(to.unsigned()),
            (F64, I32) => 0xaa + u8::from(to.unsigned()),
            (F32, I64) => 0xae + u8::from(to.unsigned()),
            (F64, I64) => 0xb0 + u8::from(to.unsigned()),
            (F64, F32) => 0xb6,
            (F32, F64) => 0xbb,
            _ => return Ok(()),
        };
        self.op(op);
        if from.float() {
            self.checked_narrow(to);
        }
        Ok(())
    }
    pub(super) fn binary(&mut self, op: &str, a: &Expr, b: &Expr) -> Result<(), String> {
        self.expr(a)?;
        if op == "&&" || op == "||" {
            self.bytes(&[0x04, I32]);
            if op == "&&" {
                self.expr(b)?;
                self.op(0x05);
                self.int(I32, 0);
            } else {
                self.int(I32, 1);
                self.op(0x05);
                self.expr(b)?;
            }
            self.op(0x0b);
            return Ok(());
        }
        let ty = val(a.ty);
        if a.ty == Type::String {
            return Err("computed string operations are unavailable".into());
        }
        if a.ty.float() {
            self.expr(b)?;
            let (base, diff) = match op {
                "+" => (0x92, 14),
                "-" => (0x93, 14),
                "*" => (0x94, 14),
                "/" => (0x95, 14),
                "==" => (0x5b, 6),
                "!=" => (0x5c, 6),
                "<" => (0x5d, 6),
                ">" => (0x5e, 6),
                "<=" => (0x5f, 6),
                ">=" => (0x60, 6),
                _ => return Err(format!("unsupported float operator {op}")),
            };
            self.op(if ty == F64 { base + diff } else { base });
            return Ok(());
        }
        let wide = ty == I64;
        let unsigned = a.ty.unsigned() || a.ty == Type::Ptr;
        if let Some(cmp) = match op {
            "==" => Some(0x46),
            "!=" => Some(0x47),
            "<" => Some(0x48 + u8::from(unsigned)),
            ">" => Some(0x4a + u8::from(unsigned)),
            "<=" => Some(0x4c + u8::from(unsigned)),
            ">=" => Some(0x4e + u8::from(unsigned)),
            _ => None,
        } {
            self.expr(b)?;
            self.op(cmp + if wide { 11 } else { 0 });
            return Ok(());
        }
        let left = self.save(ty);
        let right = self.eval_saved(b)?;
        // i32 products fit the full corresponding signed/unsigned i64 range.
        if !wide && matches!(op, "+" | "-" | "*") {
            for r in [left, right] {
                self.get(r);
                self.op(if unsigned { 0xad } else { 0xac });
            }
            self.op(match op {
                "+" => 0x7c,
                "-" => 0x7d,
                _ => 0x7e,
            });
            let from = if unsigned { Type::U64 } else { Type::I64 };
            self.cast(from, a.ty)?;
            return Ok(());
        }
        if op == "%" && !unsigned {
            self.get(left);
            self.int(ty, if wide { i64::MIN } else { i32::MIN.into() });
            self.op(if wide { 0x51 } else { 0x46 });
            self.get(right);
            self.int(ty, -1);
            self.op(if wide { 0x51 } else { 0x46 });
            self.op(0x71);
            self.trap_if();
        }
        self.get(left);
        self.get(right);
        let operator = match op {
            "+" => 0x6a,
            "-" => 0x6b,
            "*" => 0x6c,
            "/" => 0x6d + u8::from(unsigned),
            "%" => 0x6f + u8::from(unsigned),
            _ => return Err(format!("unsupported integer operator {op}")),
        };
        self.op(operator + if wide { 18 } else { 0 });
        if !wide || matches!(op, "/" | "%") {
            return Ok(());
        }
        let result = self.save(I64);
        match op {
            "+" | "-" if unsigned => {
                self.get(if op == "+" { result } else { left });
                self.get(if op == "+" { left } else { right });
                self.op(0x54);
                self.trap_if();
            }
            "+" | "-" => {
                self.get(left);
                self.get(if op == "+" { result } else { right });
                self.op(0x85);
                self.get(if op == "+" { right } else { left });
                self.get(result);
                self.op(0x85);
                self.op(0x83);
                self.int(I64, 0);
                self.op(0x53);
                self.trap_if();
            }
            "*" => {
                self.get(right);
                self.op(0x50);
                self.bytes(&[0x04, EMPTY, 0x05]);
                // MIN * -1 traps during the inverse division for one operand
                // ordering and fails the equality check for the other.
                self.get(result);
                self.get(right);
                self.op(if unsigned { 0x80 } else { 0x7f });
                self.get(left);
                self.op(0x52);
                self.trap_if();
                self.op(0x0b);
            }
            _ => unreachable!(),
        }
        self.get(result);
        Ok(())
    }
    pub(super) fn integer_method(
        &mut self,
        op: IntOp,
        a: &Expr,
        b: Option<&Expr>,
    ) -> Result<(), String> {
        self.expr(a)?;
        let ty = val(a.ty);
        let bits = match a.ty {
            Type::U8 => 8,
            Type::U16 => 16,
            _ if ty == I64 => 64,
            _ => 32,
        };
        if let Some(b) = b {
            self.expr(b)?;
            if matches!(op, IntOp::Shl | IntOp::Shr) {
                let operand = val(b.ty);
                let shift = self.save(operand);
                self.get(shift);
                self.int(operand, bits);
                self.op(if operand == I64 { 0x5a } else { 0x4f });
                self.trap_if();
                self.get(shift);
                if ty != operand {
                    self.op(if ty == I64 { 0xad } else { 0xa7 });
                }
            }
        } else {
            self.int(ty, -1);
        }
        let opcode = match op {
            IntOp::And => 0x71,
            IntOp::Or => 0x72,
            IntOp::Xor | IntOp::Not => 0x73,
            IntOp::Shl => 0x74,
            IntOp::Shr => 0x76,
            IntOp::WrappingAdd => 0x6a,
            IntOp::WrappingSub => 0x6b,
            IntOp::WrappingMul => 0x6c,
        };
        self.op(opcode + if ty == I64 { 18 } else { 0 });
        if let Some(limit) = Self::unsigned_limit(a.ty) {
            self.int(I32, limit.into());
            self.op(0x71);
        }
        Ok(())
    }
    pub(super) fn float_method(
        &mut self,
        op: FloatOp,
        a: &Expr,
        result: Type,
    ) -> Result<(), String> {
        self.expr(a)?;
        let wide = a.ty == Type::F64;
        let diff = if wide { 14 } else { 0 };
        match op {
            FloatOp::Abs => self.op(0x8b + diff),
            FloatOp::Sqrt => self.op(0x91 + diff),
            FloatOp::Floor => self.op(0x8e + diff),
            FloatOp::Ceil => self.op(0x8d + diff),
            FloatOp::RoundEven => self.op(0x90 + diff),
            FloatOp::Trunc => self.op(0x8f + diff),
            FloatOp::ToBits => self.op(if wide { 0xbd } else { 0xbc }),
            FloatOp::FromBits => self.op(if result == Type::F64 { 0xbf } else { 0xbe }),
            FloatOp::SignNegative => {
                self.op(if wide { 0xbd } else { 0xbc });
                self.int(if wide { I64 } else { I32 }, 0);
                self.op(if wide { 0x53 } else { 0x48 });
            }
            FloatOp::IsNan => {
                let t = self.save(val(a.ty));
                self.get(t);
                self.get(t);
                self.op(if wide { 0x62 } else { 0x5c });
            }
            FloatOp::IsFinite | FloatOp::IsInfinite => {
                self.op(0x8b + diff);
                if wide {
                    self.op(0x44);
                    self.bytes(&f64::INFINITY.to_le_bytes());
                } else {
                    self.op(0x43);
                    self.bytes(&f32::INFINITY.to_le_bytes());
                }
                self.op(match (wide, op) {
                    (true, FloatOp::IsFinite) => 0x63,
                    (false, FloatOp::IsFinite) => 0x5d,
                    (true, _) => 0x61,
                    _ => 0x5b,
                });
            }
            FloatOp::Round => {
                // Compare the fractional part to one half, avoiding incorrect
                // rounding of values immediately below one half.
                let t = self.save(val(a.ty));
                self.get(t);
                self.op(0x8b + diff);
                let abs = self.save(val(a.ty));
                self.get(abs);
                self.op(0x8e + diff);
                let floor = self.save(val(a.ty));
                self.get(abs);
                self.get(floor);
                self.op(0x93 + diff);
                self.float_const(wide, 0.5);
                self.op(if wide { 0x66 } else { 0x60 });
                self.bytes(&[0x04, val(a.ty)]);
                self.get(floor);
                self.float_const(wide, 1.0);
                self.op(0x92 + diff);
                self.op(0x05);
                self.get(floor);
                self.op(0x0b);
                self.get(t);
                self.op(if wide { 0xa6 } else { 0x98 });
            }
        }
        Ok(())
    }
    fn float_const(&mut self, wide: bool, value: f64) {
        if wide {
            self.op(0x44);
            self.bytes(&value.to_le_bytes());
        } else {
            self.op(0x43);
            self.bytes(&(value as f32).to_le_bytes());
        }
    }
}
