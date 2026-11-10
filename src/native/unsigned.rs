//! Checked native unsigned arithmetic. Scalar results are zero-extended to
//! 64 bits. Memory accesses and wrapping operations use their declared width.
use super::{Branch, Code};
use crate::syntax::Type;

impl Code<'_> {
    pub(super) fn unsigned_bits(&self, ty: Type) -> u32 {
        debug_assert!(ty.unsigned());
        (self.types.scalar_size(ty) * 8) as u32
    }
    pub(super) fn zero_extend(&mut self, reg: u32, bits: u32) {
        if bits < 64 {
            // UBFM Xd, Xn, #0, #(bits - 1)
            self.emit(0xd3400000 | ((bits - 1) << 10) | (reg << 5) | reg);
        }
    }
    fn check_unsigned_max(&mut self, max: u64) {
        self.imm(2, max);
        self.cmp(0, 2);
        self.branch(self.overflow, Branch::Cond(8)); // HI
    }
    pub(super) fn checked_integer_cast(&mut self, from: Type, to: Type) {
        if to.unsigned() {
            if matches!(from, Type::I32 | Type::I64) {
                self.cmp(0, 31);
                self.branch(self.overflow, Branch::Cond(11)); // LT
            }
            let bits = self.unsigned_bits(to);
            if bits < 64 && (!from.unsigned() || bits < self.unsigned_bits(from)) {
                self.check_unsigned_max((1_u64 << bits) - 1);
            }
        } else if from.unsigned() {
            let bits = match to {
                Type::I32 => 31,
                Type::I64 => 63,
                _ => return,
            };
            if self.unsigned_bits(from) > bits {
                self.check_unsigned_max((1_u64 << bits) - 1);
            }
        } else if to == Type::I32 && from == Type::I64 {
            self.narrow();
        }
    }
    // Left in x1, right in x0. All relational operators use unsigned flags.
    pub(super) fn unsigned_binary(&mut self, op: &str, ty: Type) {
        let width = self.unsigned_bits(ty);
        match op {
            "+" => {
                self.emit(0xab000020);
                if width == 64 {
                    self.branch(self.overflow, Branch::Cond(2)); // carry
                } else {
                    self.check_unsigned_max((1_u64 << width) - 1);
                }
            }
            "-" => {
                self.emit(0xeb000020);
                self.branch(self.overflow, Branch::Cond(3)); // borrow
            }
            "*" => {
                if width == 64 {
                    self.emit(0x9bc07c22); // umulh x2,x1,x0
                    self.branch(self.overflow, Branch::Nonzero(2));
                }
                self.emit(0x9b007c20);
                if width < 64 {
                    self.check_unsigned_max((1_u64 << width) - 1);
                }
            }
            "/" | "%" => {
                self.branch(self.divide_zero, Branch::Zero(0));
                self.emit(0x9ac00822); // udiv x2,x1,x0
                if op == "/" {
                    self.mov(0, 2);
                } else {
                    self.emit(0x9b008440); // msub x0,x2,x0,x1
                }
            }
            _ => {
                self.cmp(1, 0);
                let cond = match op {
                    "==" => 0,
                    "!=" => 1,
                    "<" => 3,
                    "<=" => 9,
                    ">" => 8,
                    ">=" => 2,
                    _ => unreachable!(),
                };
                self.emit(0x9a9f07e0 | ((cond ^ 1) << 12));
            }
        }
    }
}
