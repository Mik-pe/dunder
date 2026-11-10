//! Scalar IEEE binary32/binary64 operations. Expression values use raw bits
//! in x0; function boundaries use the platform's independent FP register bank.
use super::{Branch, Code};
use crate::{
    check::{Expr, FloatOp},
    syntax::Type,
};

impl Code<'_> {
    pub(super) fn fp_width(ty: Type) -> u32 {
        if ty == Type::F64 {
            1 << 22
        } else {
            0
        }
    }
    pub(super) fn bits_to_fp(&mut self, dst: u32, src: u32, ty: Type) {
        self.emit(
            (if ty == Type::F32 {
                0x1e270000
            } else {
                0x9e670000
            }) | src << 5
                | dst,
        );
    }
    pub(super) fn fp_to_bits(&mut self, dst: u32, src: u32, ty: Type) {
        self.emit(
            (if ty == Type::F32 {
                0x1e260000
            } else {
                0x9e660000
            }) | src << 5
                | dst,
        );
    }
    pub(super) fn float_binary(&mut self, op: &str, ty: Type) {
        self.bits_to_fp(0, 0, ty);
        self.bits_to_fp(1, 1, ty);
        let width = Self::fp_width(ty);
        if matches!(op, "+" | "-" | "*" | "/") {
            let base = match op {
                "+" => 0x1e202800,
                "-" => 0x1e203800,
                "*" => 0x1e200800,
                _ => 0x1e201800,
            };
            self.emit(base | width | 1 << 5);
            self.fp_to_bits(0, 0, ty);
        } else {
            self.emit(0x1e202000 | width | 1 << 5); // fcmp left,right
                                                    // Unordered: NZCV=0011. MI/LS exclude NaN for < and <=.
            let cond = match op {
                "==" => 0,
                "!=" => 1,
                "<" => 4,
                "<=" => 9,
                ">" => 12,
                ">=" => 10,
                _ => unreachable!(),
            };
            self.emit(0x9a9f07e0 | (cond ^ 1) << 12);
        }
    }
    pub(super) fn float_cast(&mut self, from: Type, to: Type) {
        if from == to {
            return;
        }
        if from.integer() && to.float() {
            self.emit(0x9e220000 | Self::fp_width(to)); // scvtf s0/d0,x0
            self.fp_to_bits(0, 0, to);
        } else if from.float() && to.float() {
            self.bits_to_fp(0, 0, from);
            self.emit(if to == Type::F32 {
                0x1e624000
            } else {
                0x1e22c000
            }); // fcvt
            self.fp_to_bits(0, 0, to);
        } else {
            self.bits_to_fp(0, 0, from);
            self.emit(0x1e25c000 | Self::fp_width(from)); // frintz
            let limit: f64 = if to == Type::I32 {
                2147483648.0
            } else {
                9223372036854775808.0
            };
            for (bound, condition) in [(-limit, 4), (limit, 10)] {
                let bits = if from == Type::F32 {
                    u64::from((bound as f32).to_bits())
                } else {
                    bound.to_bits()
                };
                self.imm(1, bits);
                self.bits_to_fp(1, 1, from);
                self.emit(0x1e202000 | Self::fp_width(from) | 1 << 16); // fcmp value,bound
                self.branch(self.float_conversion, Branch::Cond(6)); // NaN
                self.branch(self.float_conversion, Branch::Cond(condition));
            }
            self.emit(0x9e380000 | Self::fp_width(from)); // fcvtzs x0,s0/d0
        }
    }
    pub(super) fn float_expr(&mut self, op: FloatOp, value: &Expr, result: Type) {
        self.expr(value);
        if matches!(op, FloatOp::FromBits) {
            if result == Type::F32 {
                self.emit(0x2a0003e0);
            }
            return;
        }
        if matches!(op, FloatOp::ToBits) {
            if value.ty == Type::F32 {
                self.emit(0x93407c00);
            }
            return;
        }
        if matches!(op, FloatOp::SignNegative) {
            let shift = if value.ty == Type::F32 { 31 } else { 63 };
            self.emit(0xd340fc00 | shift << 16);
            return;
        }
        self.bits_to_fp(0, 0, value.ty);
        let width = Self::fp_width(value.ty);
        match op {
            FloatOp::IsNan => {
                self.emit(0x1e202000 | width);
                self.emit(0x9a9f07e0 | 7 << 12); // cset x0,vs
                return;
            }
            FloatOp::IsFinite | FloatOp::IsInfinite => {
                self.emit(0x1e20c000 | width); // fabs
                self.imm(
                    1,
                    if value.ty == Type::F32 {
                        u64::from(f32::INFINITY.to_bits())
                    } else {
                        f64::INFINITY.to_bits()
                    },
                );
                self.bits_to_fp(1, 1, value.ty);
                self.emit(0x1e202000 | width | 1 << 16);
                let cond = if matches!(op, FloatOp::IsFinite) {
                    4
                } else {
                    0
                };
                self.emit(0x9a9f07e0 | (cond ^ 1) << 12);
                return;
            }
            _ => {}
        }
        let base = match op {
            FloatOp::Abs => 0x1e20c000,
            FloatOp::Sqrt => 0x1e21c000,
            FloatOp::Floor => 0x1e254000,
            FloatOp::Ceil => 0x1e24c000,
            FloatOp::Round => 0x1e264000,
            FloatOp::RoundEven => 0x1e244000,
            FloatOp::Trunc => 0x1e25c000,
            _ => unreachable!(),
        };
        self.emit(base | width);
        self.fp_to_bits(0, 0, value.ty);
    }
    pub(super) fn float_to_string(&mut self, ty: Type) {
        self.imm(1, u64::from(ty == Type::F32));
        self.branch(
            self.float_formatter
                .expect("type checking installs the float formatter"),
            Branch::Call,
        );
    }
}
