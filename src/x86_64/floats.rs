//! SSE2 scalar floating point lowering. Rounding uses the IEEE bit layout,
//! so the generated programs do not require SSE4.1 or a platform math library.
use super::Lower;

#[derive(Clone, Copy)]
enum Rounding {
    Trunc,
    Floor,
    Ceil,
    Away,
    Even,
}

impl Lower {
    fn float_jump(&mut self, condition: Option<u8>) -> usize {
        match condition {
            Some(c) => self.emit(&[0x0f, 0x80 | c]),
            None => self.emit(&[0xe9]),
        }
        let at = self.code.len();
        self.disp32(0);
        at
    }

    fn float_patch(&mut self, at: usize) {
        let relative = self.code.len() as i64 - (at + 4) as i64;
        self.code[at..at + 4].copy_from_slice(&(relative as i32).to_le_bytes());
    }

    fn float_bits_load(&mut self, register: u8, arm: u32, wide: bool) {
        self.rex(wide, register, 0, 15);
        self.emit(&[0x8b, 0x87 | ((register & 7) << 3)]);
        self.disp32(256 + arm as i32 * 8);
    }

    fn float_bits_store(&mut self, arm: u32, register: u8, wide: bool) {
        self.rex(wide, register, 0, 15);
        self.emit(&[0x89, 0x87 | ((register & 7) << 3)]);
        self.disp32(256 + arm as i32 * 8);
    }

    fn float_flags(&mut self, flags: u32) {
        self.emit(&[0x49, 0xc7, 0x87]); // mov qword [r15+512], flags
        self.disp32(512);
        self.disp32(flags as i32);
    }

    fn float_compare(&mut self, left: u32, right: u32, wide: bool) {
        self.fp_load(0, left, wide);
        self.fp_load(1, right, wide);
        if wide {
            self.emit(&[0x66]);
        }
        self.emit(&[0x0f, 0x2e, 0xc1]); // ucomis{s,d} xmm0,xmm1
        let unordered = self.float_jump(Some(10)); // jp
        let less = self.float_jump(Some(2)); // jb
        let equal = self.float_jump(Some(4)); // je
        self.float_flags(1); // ARM NZCV = 0010
        let greater_done = self.float_jump(None);
        self.float_patch(less);
        self.float_flags(0x80); // ARM NZCV = 1000
        let less_done = self.float_jump(None);
        self.float_patch(equal);
        self.float_flags(0x41); // ARM NZCV = 0110
        let equal_done = self.float_jump(None);
        self.float_patch(unordered);
        self.float_flags(0x801); // ARM NZCV = 0011
        for at in [greater_done, less_done, equal_done] {
            self.float_patch(at);
        }
    }

    fn float_round_bits(&mut self, dst: u32, src: u32, wide: bool, mode: Rounding) {
        let fraction_bits = if wide { 52 } else { 23 };
        let bias = if wide { 1023 } else { 127 };
        let sign = if wide { 1u64 << 63 } else { 1u64 << 31 };
        let one = if wide { 0x3ff0000000000000 } else { 0x3f800000 };
        let half = if wide { 0x3fe0000000000000 } else { 0x3f000000 };
        self.float_bits_load(0, src, wide);
        self.emit(&[0x48, 0x89, 0xc2]); // rdx = original
        self.imm64(8, sign);
        self.emit(&[0x4c, 0x21, 0xc2]); // rdx = sign
        self.imm64(8, sign - 1);
        self.emit(&[0x4c, 0x21, 0xc0]); // rax = magnitude
        self.emit(&[0x48, 0x89, 0xc1, 0x48, 0xc1, 0xe9, fraction_bits]);
        self.emit(&[0x48, 0x81, 0xf9]); // cmp exponent,bias+fraction_bits
        self.disp32(bias + i32::from(fraction_bits));
        let mut done = vec![self.float_jump(Some(3))]; // jae: integral, inf, NaN
        self.emit(&[0x48, 0x81, 0xf9]);
        self.disp32(bias);
        let small = self.float_jump(Some(2)); // exponent < bias

        // 1 <= magnitude < 2^fraction_bits. The increment in the raw bit
        // representation is 1 << (fraction_bits - unbiased_exponent).
        self.imm64(8, (bias + i32::from(fraction_bits)) as u64);
        self.emit(&[0x49, 0x29, 0xc8, 0x4c, 0x89, 0xc1]); // rcx = shift
        self.imm64(8, 1);
        self.emit(&[0x49, 0xd3, 0xe0]); // shl r8,cl: increment
        self.emit(&[0x4d, 0x89, 0xc1, 0x49, 0xff, 0xc9]); // r9 = mask
        self.emit(&[0x49, 0x89, 0xc2, 0x4d, 0x21, 0xca]); // r10 = fractional bits
        self.emit(&[0x49, 0xf7, 0xd1, 0x4c, 0x21, 0xc8]); // clear fractional bits
        match mode {
            Rounding::Trunc => {}
            Rounding::Floor | Rounding::Ceil => {
                self.emit(&[0x48, 0x85, 0xd2]);
                done.push(self.float_jump(Some(if matches!(mode, Rounding::Floor) {
                    4
                } else {
                    5
                })));
                self.emit(&[0x4d, 0x85, 0xd2]);
                done.push(self.float_jump(Some(4)));
                self.emit(&[0x4c, 0x01, 0xc0]);
            }
            Rounding::Away | Rounding::Even => {
                self.emit(&[0x4d, 0x89, 0xc1, 0x49, 0xd1, 0xe9]); // half increment
                self.emit(&[0x4d, 0x39, 0xca]); // cmp fractional,half
                done.push(self.float_jump(Some(2)));
                if matches!(mode, Rounding::Even) {
                    let above_half = self.float_jump(Some(7)); // ja
                    self.emit(&[0x4c, 0x85, 0xc0]); // test integral,increment
                    done.push(self.float_jump(Some(4)));
                    self.float_patch(above_half);
                }
                self.emit(&[0x4c, 0x01, 0xc0]);
            }
        }
        done.push(self.float_jump(None));

        self.float_patch(small);
        // |value| < 1. Preserve the sign even when the result is zero.
        let mut zero = Vec::new();
        match mode {
            Rounding::Trunc => zero.push(self.float_jump(None)),
            Rounding::Floor | Rounding::Ceil => {
                self.emit(&[0x48, 0x85, 0xd2]);
                zero.push(self.float_jump(Some(if matches!(mode, Rounding::Floor) {
                    4
                } else {
                    5
                })));
                self.emit(&[0x48, 0x85, 0xc0]);
                zero.push(self.float_jump(Some(4)));
            }
            Rounding::Away | Rounding::Even => {
                self.imm64(8, half);
                self.emit(&[0x4c, 0x39, 0xc0]);
                zero.push(self.float_jump(Some(if matches!(mode, Rounding::Away) {
                    2
                } else {
                    6
                })));
            }
        }
        self.imm64(0, one);
        done.push(self.float_jump(None));
        for at in zero {
            self.float_patch(at);
        }
        self.emit(&[0x31, 0xc0]);
        for at in done {
            self.float_patch(at);
        }
        self.emit(&[0x48, 0x09, 0xd0]); // restore sign
        self.float_bits_store(dst, 0, wide);
    }

    pub(super) fn lower_float(&mut self, word: u32) -> bool {
        let dst = word & 31;
        let src = (word >> 5) & 31;
        let right = (word >> 16) & 31;
        let wide = word & (1 << 22) != 0;
        match word & 0xfffffc00 {
            0x1e270000 | 0x9e670000 => {
                self.load(0, src);
                self.float_bits_store(dst, 0, wide);
                return true;
            }
            0x1e260000 | 0x9e660000 => {
                self.float_bits_load(0, src, wide);
                self.store(dst, 0);
                return true;
            }
            0x9e220000 | 0x9e620000 => {
                self.load(0, src);
                self.emit(&[if wide { 0xf2 } else { 0xf3 }, 0x48, 0x0f, 0x2a, 0xc0]);
                self.fp_store(dst, 0, wide);
                return true;
            }
            0x9e230000 | 0x9e630000 => {
                let prefix = if wide { 0xf2 } else { 0xf3 };
                self.load(0, src);
                self.emit(&[0x48, 0x85, 0xc0]); // test rax,rax
                let small = self.float_jump(Some(9)); // jns
                                                      // Fold the low bit into the half before rounding. This preserves
                                                      // ties-to-even for the subsequent exact multiplication by two.
                self.emit(&[0x48, 0x89, 0xc1, 0x83, 0xe1, 1]);
                self.emit(&[0x48, 0xd1, 0xe8, 0x48, 0x09, 0xc8]);
                self.emit(&[prefix, 0x48, 0x0f, 0x2a, 0xc0]);
                self.emit(&[prefix, 0x0f, 0x58, 0xc0]); // addss/addsd xmm0,xmm0
                let done = self.float_jump(None);
                self.float_patch(small);
                self.emit(&[prefix, 0x48, 0x0f, 0x2a, 0xc0]);
                self.float_patch(done);
                self.fp_store(dst, 0, wide);
                return true;
            }
            0x9e390000 | 0x9e790000 => {
                // The caller has proved 0 <= value < 2^64. SSE2 converts
                // signed 64-bit values, so subtract 2^63 in the upper half.
                self.fp_load(0, src, wide);
                let prefix = if wide { 0xf2 } else { 0xf3 };
                let limit = if wide { 0x43e0000000000000 } else { 0x5f000000 };
                self.imm64(0, limit);
                self.emit(&[0x66, 0x48, 0x0f, 0x6e, 0xc8]); // movq xmm1,rax
                if wide {
                    self.emit(&[0x66]);
                }
                self.emit(&[0x0f, 0x2e, 0xc1]); // ucomiss/ucomisd xmm0,xmm1
                let small = self.float_jump(Some(2)); // jb
                self.emit(&[prefix, 0x0f, 0x5c, 0xc1]); // subss/subsd xmm0,xmm1
                self.emit(&[prefix, 0x48, 0x0f, 0x2c, 0xc0]);
                self.emit(&[0x48, 0x0f, 0xba, 0xe8, 63]); // bts rax,63
                let done = self.float_jump(None);
                self.float_patch(small);
                self.emit(&[prefix, 0x48, 0x0f, 0x2c, 0xc0]);
                self.float_patch(done);
                self.store(dst, 0);
                return true;
            }
            0x9e380000 | 0x9e780000 => {
                self.fp_load(0, src, wide);
                self.emit(&[if wide { 0xf2 } else { 0xf3 }, 0x48, 0x0f, 0x2c, 0xc0]);
                self.store(dst, 0);
                return true;
            }
            0x1e624000 | 0x1e22c000 => {
                self.fp_load(0, src, wide);
                self.emit(&[if wide { 0xf2 } else { 0xf3 }, 0x0f, 0x5a, 0xc0]);
                self.fp_store(dst, 0, !wide);
                return true;
            }
            _ => {}
        }
        let arithmetic = match word & 0xff20fc00 {
            0x1e202800 => Some(0x58),
            0x1e203800 => Some(0x5c),
            0x1e200800 => Some(0x59),
            0x1e201800 => Some(0x5e),
            _ => None,
        };
        if let Some(opcode) = arithmetic {
            self.fp_load(0, src, wide);
            self.fp_load(1, right, wide);
            self.emit(&[if wide { 0xf2 } else { 0xf3 }, 0x0f, opcode, 0xc1]);
            self.fp_store(dst, 0, wide);
            return true;
        }
        if word & 0xff20fc1f == 0x1e202000 {
            self.float_compare(src, right, wide);
            return true;
        }
        match word & 0xffbffc00 {
            0x1e20c000 | 0x1e214000 => {
                self.float_bits_load(0, src, wide);
                let sign = if wide { 1u64 << 63 } else { 1u64 << 31 };
                if word & 0xffbffc00 == 0x1e20c000 {
                    self.imm64(1, sign - 1);
                    self.emit(&[0x48, 0x21, 0xc8]);
                } else {
                    self.imm64(1, sign);
                    self.emit(&[0x48, 0x31, 0xc8]);
                }
                self.float_bits_store(dst, 0, wide);
                true
            }
            0x1e21c000 => {
                self.fp_load(0, src, wide);
                self.emit(&[if wide { 0xf2 } else { 0xf3 }, 0x0f, 0x51, 0xc0]);
                self.fp_store(dst, 0, wide);
                true
            }
            0x1e254000 | 0x1e24c000 | 0x1e264000 | 0x1e244000 | 0x1e25c000 => {
                let rounding = match word & 0xffbffc00 {
                    0x1e254000 => Rounding::Floor,
                    0x1e24c000 => Rounding::Ceil,
                    0x1e264000 => Rounding::Away,
                    0x1e244000 => Rounding::Even,
                    _ => Rounding::Trunc,
                };
                self.float_round_bits(dst, src, wide, rounding);
                true
            }
            _ => false,
        }
    }
}
