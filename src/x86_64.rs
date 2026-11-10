//! Native x86-64 lowering for the bootstrap backend's instruction vocabulary.
//! Lowering happens entirely at compile time; generated programs contain only
//! x86-64 instructions, data, Linux system calls, and optional native C calls.
//! Hot virtual registers have fixed host-register homes; the remaining register
//! bank lives in a private allocation. Fixed homes work across every control-flow
//! edge without a runtime dispatcher, cache flush or data-flow analysis.
use crate::native::{Branch, Label};
mod floats;
mod syscall;

struct Lower {
    code: Vec<u8>,
    relocations: Vec<(usize, usize)>,
    stack_failures: Vec<usize>,
    globals: Vec<(usize, usize)>,
}
impl Lower {
    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }
    fn rex(&mut self, wide: bool, r: u8, x: u8, b: u8) {
        let rex = 0x40 | (u8::from(wide) << 3) | ((r >> 3) << 2) | ((x >> 3) << 1) | (b >> 3);
        if rex != 0x40 {
            self.code.push(rex);
        }
    }
    fn modrm_reg(&mut self, reg: u8, rm: u8) {
        self.code.push(0xc0 | ((reg & 7) << 3) | (rm & 7));
    }
    fn disp32(&mut self, n: i32) {
        self.emit(&n.to_le_bytes());
    }
    fn imm64(&mut self, reg: u8, value: u64) {
        // MOV preserves flags. A 32-bit write zero-extends, and C7 /0 sign-
        // extends its immediate; reserve MOVABS for actual 64-bit constants.
        if let Ok(value) = u32::try_from(value) {
            self.rex(false, 0, 0, reg);
            self.code.push(0xb8 | (reg & 7));
            self.emit(&value.to_le_bytes());
            return;
        }
        if (value as i64) == i64::from(value as i32) {
            self.rex(true, 0, 0, reg);
            self.code.push(0xc7);
            self.modrm_reg(0, reg);
            self.emit(&(value as u32).to_le_bytes());
            return;
        }
        self.rex(true, 0, 0, reg);
        self.code.push(0xb8 | (reg & 7));
        self.emit(&value.to_le_bytes());
    }
    // The arithmetic lowerer uses rax/rcx/rdx and r8..r11 as scratch registers.
    // These fixed homes are callee-saved in SysV and preserved by Linux syscalls.
    // Native calls preserve rbx while temporarily using it to align the host stack.
    fn register_home(offset: i32) -> Option<u8> {
        match offset {
            0 => Some(12),  // x0: expression results and first argument
            8 => Some(13),  // x1: left operand and second argument
            16 => Some(14), // x2: arithmetic/address scratch and third argument
            200 => Some(3), // x25: hottest address-free scalar in a loop
            248 => Some(5), // virtual SP, distinct from the host call stack
            _ => None,
        }
    }
    fn mov_reg(&mut self, dst: u8, src: u8) {
        if dst != src {
            self.rex(true, src, 0, dst);
            self.code.push(0x89);
            self.modrm_reg(src, dst);
        }
    }
    fn state_load(&mut self, reg: u8, offset: i32) {
        if let Some(home) = Self::register_home(offset) {
            self.mov_reg(reg, home);
            return;
        }
        self.rex(true, reg, 0, 15);
        self.emit(&[0x8b, 0x87 | ((reg & 7) << 3)]);
        self.disp32(offset);
    }
    fn state_store(&mut self, offset: i32, reg: u8) {
        if offset == 248 {
            // Bounds-check every value-stack update, including large frame
            // adjustments that could jump over a guard page.
            self.rex(true, reg, 0, 11);
            self.code.push(0x89);
            self.modrm_reg(reg, 11); // mov r11,source
            self.emit(&[0x4d, 0x29, 0xfb]); // sub r11,r15
            self.emit(&[0x49, 0x81, 0xfb]);
            self.disp32(8192);
            let below = self.local_jump(Some(2));
            self.stack_failures.push(below);
            self.emit(&[0x49, 0x81, 0xfb]);
            self.disp32(64 * 1024 * 1024);
            let above = self.local_jump(Some(7));
            self.stack_failures.push(above);
        }
        if let Some(home) = Self::register_home(offset) {
            self.mov_reg(home, reg);
            return;
        }
        self.rex(true, reg, 0, 15);
        self.emit(&[0x89, 0x87 | ((reg & 7) << 3)]);
        self.disp32(offset);
    }
    fn load(&mut self, reg: u8, arm: u32) {
        if arm == 31 {
            self.imm64(reg, 0);
        } else {
            self.state_load(reg, (arm * 8) as i32);
        }
    }
    fn store(&mut self, arm: u32, reg: u8) {
        if arm != 31 {
            self.state_store((arm * 8) as i32, reg);
        }
    }
    fn base(&mut self, reg: u8, arm: u32) {
        self.state_load(reg, (arm * 8) as i32);
    }
    fn fp_load(&mut self, xmm: u8, arm: u32, wide: bool) {
        self.code.push(if wide { 0xf2 } else { 0xf3 });
        self.rex(false, xmm, 0, 15);
        self.emit(&[0x0f, 0x10, 0x87 | (xmm & 7) << 3]);
        self.disp32(256 + (arm * 8) as i32);
    }
    fn fp_store(&mut self, arm: u32, xmm: u8, wide: bool) {
        // A write to S clears the upper bits of its scalar storage.
        if !wide {
            self.imm64(11, 0);
            self.state_store(256 + (arm * 8) as i32, 11);
        }
        self.code.push(if wide { 0xf2 } else { 0xf3 });
        self.rex(false, xmm, 0, 15);
        self.emit(&[0x0f, 0x11, 0x87 | (xmm & 7) << 3]);
        self.disp32(256 + (arm * 8) as i32);
    }
    fn save_flags(&mut self, invert_carry: bool) {
        self.emit(&[0x9c, 0x41, 0x5b]); // pushfq; pop r11
        if invert_carry {
            self.emit(&[0x49, 0x83, 0xf3, 1]);
        }
        self.state_store(512, 11);
    }
    fn restore_flags(&mut self) {
        self.state_load(11, 512);
        self.emit(&[0x41, 0x53, 0x9d]);
    }
    // ARM's saved carry means no borrow; signed conditions share x86 flags.
    fn condition(&mut self, condition: u32) -> u8 {
        self.restore_flags();
        match condition {
            0 => 4,
            1 => 5,
            2 => 2,
            3 => 3,
            4 => 8,
            5 => 9,
            6 => 0,
            7 => 1,
            8 => {
                self.code.push(0xf5);
                7
            }
            9 => {
                self.code.push(0xf5);
                6
            }
            10 => 13,
            11 => 12,
            12 => 15,
            13 => 14,
            _ => unreachable!("unsupported ARM condition"),
        }
    }
    fn relative(&mut self, arm_index: usize) {
        self.relocations.push((self.code.len(), arm_index));
        self.disp32(0);
    }
    fn branch(&mut self, branch: &Branch, arm_index: usize) {
        match *branch {
            Branch::Jump => {
                self.code.push(0xe9);
                self.relative(arm_index);
            }
            Branch::Call => {
                self.code.push(0xe8);
                self.relative(arm_index);
            }
            Branch::Cond(c) => {
                let c = self.condition(c);
                self.emit(&[0x0f, 0x80 | c]);
                self.relative(arm_index);
            }
            Branch::Zero(r) | Branch::Nonzero(r) => {
                self.load(0, r);
                self.emit(&[
                    0x48,
                    0x85,
                    0xc0,
                    0x0f,
                    if matches!(branch, Branch::Zero(_)) {
                        0x84
                    } else {
                        0x85
                    },
                ]);
                self.relative(arm_index);
            }
            Branch::Address(r) => {
                self.emit(&[0x48, 0x8d, 0x05]);
                self.relative(arm_index);
                self.store(r, 0);
            }
        }
    }
    fn local_jump(&mut self, cc: Option<u8>) -> usize {
        if let Some(cc) = cc {
            self.emit(&[0x0f, 0x80 | cc]);
        } else {
            self.code.push(0xe9);
        }
        let at = self.code.len();
        self.disp32(0);
        at
    }
    fn patch_here(&mut self, at: usize) {
        let delta = (self.code.len() as i64 - (at + 4) as i64) as i32;
        self.code[at..at + 4].copy_from_slice(&delta.to_le_bytes());
    }
    fn shift(&mut self, reg: u8, op: u8, count: u8, wide: bool) {
        if count != 0 {
            self.rex(wide, 0, 0, reg);
            self.code.push(0xc1);
            self.modrm_reg(op, reg);
            self.code.push(count);
        }
    }
    // Address is in rax; transfer between memory and rcx.
    fn memory(&mut self, load: bool, size: u32, signed: bool, offset: i32) {
        if load {
            match (size, signed) {
                (0, false) | (1, false) => {
                    self.emit(&[0x0f, if size == 0 { 0xb6 } else { 0xb7 }, 0x88]);
                }
                (2, true) => self.emit(&[0x48, 0x63, 0x88]),
                (2, false) => self.emit(&[0x8b, 0x88]),
                (3, _) => self.emit(&[0x48, 0x8b, 0x88]),
                _ => unreachable!(),
            }
        } else {
            if size == 1 {
                self.code.push(0x66);
            }
            if size == 3 {
                self.code.push(0x48);
            }
            self.emit(&[if size == 0 { 0x88 } else { 0x89 }, 0x88]);
        }
        self.disp32(offset);
    }
    /// Private emitter operation, not an instruction placed in an ARM image.
    /// Register arguments occupy virtual x0..x5 and v0..v7. Spills are packed on the value stack.
    fn external_call(
        &mut self,
        function: u32,
        argc: usize,
        floats: usize,
        callbacks: bool,
    ) -> Result<(), String> {
        if argc > 6 || floats > 8 || function == 31 {
            return Err("invalid native C call shape".into());
        }
        // Dunder's call stack has no host-ABI alignment contract. Remember it
        // in a callee-saved register, then build a properly aligned SysV frame.
        self.emit(&[0x53, 0x48, 0x89, 0xe3]); // push rbx; mov rbx,rsp
        if callbacks {
            // Preserve the previous suspended context for nested C -> Dunder
            // -> C calls. rbx holds our private host frame across C.
            self.load(10, 15);
            self.emit(&[0x41, 0x52, 0x41, 0xff, 0x32, 0x41, 0xff, 0x72, 8]); // push r10,[r10],[r10+8]
            self.emit(&[0x4d, 0x89, 0x3a, 0x49, 0x89, 0x6a, 8]); // [r10]=r15, [r10+8]=rbp
        }
        self.emit(&[0x48, 0x83, 0xe4, 0xf0]); // and rsp,-16
                                              // Copy the ABI-classified outgoing arguments to the aligned C stack.
        self.load(1, 16);
        self.emit(&[0x48, 0x29, 0xcc]); // sub rsp,rcx
        self.mov_reg(6, 5);
        self.mov_reg(7, 4);
        self.emit(&[0xf3, 0xa4]); // rep movsb
        self.load(11, function);
        for (arg, reg) in [7, 6, 2, 1, 8, 9].into_iter().enumerate().take(argc) {
            self.load(reg, arg as u32);
        }
        for arg in 0..floats {
            self.fp_load(arg as u8, arg as u32, true);
        }
        self.emit(&[0xb8]); // SysV's vector-argument count, also for variadics.
        self.disp32(floats as i32);
        self.emit(&[0x41, 0xff, 0xd3]); // call r11
        self.store(0, 0);
        self.fp_store(0, 0, true);
        self.fp_store(1, 1, true); // second SysV aggregate return eightbyte
        if callbacks {
            self.emit(&[
                0x4c, 0x8b, 0x53, 0xf8, 0x48, 0x8b, 0x43, 0xf0, 0x49, 0x89, 0x02,
            ]);
            self.emit(&[0x48, 0x8b, 0x43, 0xe8, 0x49, 0x89, 0x42, 8]);
        }
        self.emit(&[0x48, 0x89, 0xdc, 0x5b]); // mov rsp,rbx; pop rbx
        Ok(())
    }
    fn word(&mut self, w: u32) -> Result<(), String> {
        let rd = w & 31;
        let rn = (w >> 5) & 31;
        let rm = (w >> 16) & 31;
        let wide = w >> 31 != 0;
        if w == 0xd53bd04a {
            self.emit(&[0x64, 0x48, 0x8b, 0x04, 0x25, 0, 0, 0, 0]); // mov rax,fs:[0]
            self.store(10, 0);
            return Ok(());
        }
        if w >> 24 == 0xfd {
            self.emit(&[0x53, 0x55, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57]);
            self.emit(&[0x48, 0x8d, 0x05]); // lea rax,[rip+callback context]
            self.globals
                .push((self.code.len(), (w & 0xffffff) as usize));
            self.disp32(0);
            self.emit(&[0x64, 0x4c, 0x8b, 0x1c, 0x25, 0, 0, 0, 0]); // mov r11,fs:[0]
            self.emit(&[0x4c, 0x3b, 0x58, 48]); // cmp r11,[rax+active pthread]
            let active = self.local_jump(Some(4));
            self.imm64(7, 111);
            self.imm64(0, 231);
            self.emit(&[0x0f, 0x05, 0x0f, 0x0b]);
            self.patch_here(active);
            self.emit(&[0x4c, 0x8b, 0x38, 0x48, 0x8b, 0x68, 8]); // r15,rbp = suspended Dunder state
            for (arg, reg) in [7, 6, 2, 1, 8, 9].into_iter().enumerate() {
                self.store(arg as u32, reg);
            }
            for fp in 0..8 {
                self.fp_store(fp, fp as u8, true);
            }
            self.emit(&[0x48, 0x8d, 0x44, 0x24, 56]); // address of first on-stack C argument
            self.store(14, 0);
            return Ok(());
        }
        if w == 0xfb000000 {
            self.load(0, 0);
            self.fp_load(0, 0, true);
            self.emit(&[
                0x41, 0x5f, 0x41, 0x5e, 0x41, 0x5d, 0x41, 0x5c, 0x5d, 0x5b, 0xc3,
            ]);
            return Ok(());
        }
        if matches!(w & 0xfff0_e0e0, 0xfe00_0000 | 0xfc00_0000) {
            return self.external_call(
                w & 31,
                ((w >> 8) & 255) as usize,
                ((w >> 16) & 255) as usize,
                w >> 24 == 0xfc,
            );
        }
        if self.lower_float(w) {
            return Ok(());
        }
        match w {
            0xd503201f | 0xd51b441f => return Ok(()), // NOP; default IEEE FP state
            0xd65f03c0 => {
                self.code.push(0xc3);
                return Ok(());
            }
            0xd4200000 => {
                self.emit(&[0x0f, 0x0b]);
                return Ok(());
            }
            0xd4000001 => {
                self.syscall();
                return Ok(());
            }
            _ => {}
        }
        if w & 0xfffffc1f == 0xd63f0000 {
            // BLR
            self.load(0, rn);
            self.emit(&[0xff, 0xd0]);
            return Ok(());
        }
        if w & 0x7f800000 == 0x52800000 {
            // MOVZ
            self.imm64(0, ((w >> 5 & 0xffff) as u64) << ((w >> 21 & 3) * 16));
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0x7f800000 == 0x72800000 {
            // MOVK
            let shift = (w >> 21 & 3) * 16;
            self.load(0, rd);
            self.imm64(1, !(0xffff_u64 << shift));
            self.emit(&[0x48, 0x21, 0xc8]);
            self.imm64(1, ((w >> 5 & 0xffff) as u64) << shift);
            self.emit(&[0x48, 0x09, 0xc8]);
            if !wide {
                self.emit(&[0x89, 0xc0]);
            }
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0x1f000000 == 0x11000000 {
            // ADD/SUB immediate (SP allowed)
            let subtract = w & (1 << 30) != 0;
            let flags = w & (1 << 29) != 0;
            let imm = (w >> 10 & 4095) << if w & (1 << 22) != 0 { 12 } else { 0 };
            self.base(0, rn);
            self.rex(wide, 0, 0, 0);
            self.code.push(if subtract { 0x2d } else { 0x05 });
            self.disp32(imm as i32);
            if flags {
                self.save_flags(subtract);
                self.store(rd, 0);
            } else {
                self.state_store((rd * 8) as i32, 0);
            }
            return Ok(());
        }
        if w & 0x1f200000 == 0x0b000000 {
            // ADD/SUB shifted register
            let subtract = w & (1 << 30) != 0;
            let flags = w & (1 << 29) != 0;
            self.load(0, rn);
            self.load(1, rm);
            let amount = (w >> 10 & 63) as u8;
            self.shift(
                1,
                match w >> 22 & 3 {
                    0 => 4,
                    1 => 5,
                    2 => 7,
                    _ => return Err("unsupported shifted add".into()),
                },
                amount,
                wide,
            );
            self.rex(wide, 1, 0, 0);
            self.code.push(if subtract { 0x29 } else { 0x01 });
            self.modrm_reg(1, 0);
            if flags {
                self.save_flags(subtract);
            }
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0x1f000000 == 0x0a000000 {
            // logical shifted register
            self.load(0, rn);
            self.load(1, rm);
            let amount = (w >> 10 & 63) as u8;
            self.shift(
                1,
                match w >> 22 & 3 {
                    0 => 4,
                    1 => 5,
                    2 => 7,
                    3 => 1,
                    _ => unreachable!(),
                },
                amount,
                wide,
            );
            if w & (1 << 21) != 0 {
                self.rex(wide, 0, 0, 1);
                self.emit(&[0xf7, 0xd1]);
            }
            let op = w >> 29 & 3;
            self.rex(wide, 1, 0, 0);
            self.code.push(match op {
                0 | 3 => 0x21,
                1 => 0x09,
                2 => 0x31,
                _ => unreachable!(),
            });
            self.modrm_reg(1, 0);
            if op == 3 {
                self.save_flags(false);
            }
            self.store(rd, 0);
            return Ok(());
        }
        if matches!(w & 0x7f800000, 0x13000000 | 0x53000000) {
            // SBFM/UBFM
            let signed = w & 0x40000000 == 0;
            let width = if wide { 64 } else { 32 };
            let r = (w >> 16 & 63) as u8;
            let s = (w >> 10 & 63) as u8;
            self.load(0, rn);
            if s >= r {
                self.shift(0, 4, width - 1 - s, wide);
                self.shift(0, if signed { 7 } else { 5 }, width - 1 - s + r, wide);
            } else {
                self.imm64(1, (1_u64 << (s + 1)) - 1);
                self.emit(&[0x48, 0x21, 0xc8]);
                self.shift(0, 4, width - r, wide);
                if signed {
                    return Err("unsupported signed wrapping bitfield".into());
                }
            }
            if !wide {
                self.emit(&[0x89, 0xc0]);
            }
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0x1f000000 == 0x1b000000 {
            // MADD/MSUB/SMULH
            let ra = w >> 10 & 31;
            self.load(0, rn);
            self.load(1, rm);
            if matches!(w & 0x00e00000, 0x00400000 | 0x00c00000) {
                // IMUL/MUL r/m64: the high half is placed in RDX.
                let unsigned = w & 0x00800000 != 0;
                self.emit(&[0x48, 0xf7, if unsigned { 0xe1 } else { 0xe9 }]);
                self.store(rd, 2);
            } else if w & 0x00e00000 == 0 {
                self.rex(wide, 0, 0, 1);
                self.emit(&[0x0f, 0xaf, 0xc1]);
                self.load(1, ra);
                self.rex(wide, 0, 0, 1);
                if w & (1 << 15) != 0 {
                    self.emit(&[0x29, 0xc1]);
                    self.store(rd, 1);
                } else {
                    self.emit(&[0x01, 0xc8]);
                    self.store(rd, 0);
                }
            } else {
                return Err(format!("unsupported multiply 0x{w:08x}"));
            }
            return Ok(());
        }
        if w & 0x1fe00000 == 0x1ac00000 {
            let op = w >> 10 & 63;
            self.load(0, rn);
            self.load(1, rm);
            match op {
                8..=10 => {
                    // LSLV/LSRV/ASRV
                    self.rex(wide, 0, 0, 0);
                    self.code.push(0xd3);
                    self.modrm_reg(
                        if op == 8 {
                            4
                        } else if op == 9 {
                            5
                        } else {
                            7
                        },
                        0,
                    );
                }
                2 | 3 => {
                    // unsigned/signed divide, including architectural exceptional cases
                    self.rex(wide, 1, 0, 1);
                    self.emit(&[0x85, 0xc9]);
                    let zero = self.local_jump(Some(4));
                    let mut overflow = None;
                    if op == 3 {
                        self.imm64(
                            2,
                            if wide {
                                i64::MIN as u64
                            } else {
                                i32::MIN as u32 as u64
                            },
                        );
                        self.rex(wide, 2, 0, 0);
                        self.emit(&[0x39, 0xd0]);
                        let normal = self.local_jump(Some(5));
                        self.rex(wide, 0, 0, 1);
                        self.emit(&[0x83, 0xf9, 0xff]);
                        overflow = Some(self.local_jump(Some(4)));
                        self.patch_here(normal);
                        if wide {
                            self.code.push(0x48);
                        }
                        self.code.push(0x99); // CQO/CDQ
                    } else {
                        self.emit(&[0x31, 0xd2]);
                    }
                    self.rex(wide, 0, 0, 1);
                    self.emit(&[0xf7, if op == 3 { 0xf9 } else { 0xf1 }]);
                    let done = self.local_jump(None);
                    self.patch_here(zero);
                    self.emit(&[0x31, 0xc0]);
                    self.patch_here(done);
                    if let Some(at) = overflow {
                        self.patch_here(at);
                    }
                }
                _ => return Err(format!("unsupported binary instruction 0x{w:08x}")),
            }
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0xffff0fe0 == 0x9a9f07e0 {
            // CSET alias
            let condition = ((w >> 12) & 15) ^ 1;
            let cc = self.condition(condition);
            self.emit(&[0x0f, 0x90 | cc, 0xc0, 0x0f, 0xb6, 0xc0]);
            self.store(rd, 0);
            return Ok(());
        }
        if w & 0x3b000000 == 0x39000000 {
            // unsigned-offset load/store
            let size = w >> 30;
            let op = w >> 22 & 3;
            if op > 2 || (op == 2 && size != 2) {
                return Err(format!("unsupported load/store 0x{w:08x}"));
            }
            self.base(0, rn);
            let offset = ((w >> 10 & 4095) << size) as i32;
            if op == 0 {
                self.load(1, rd);
            }
            self.memory(op != 0, size, op == 2, offset);
            if op != 0 {
                self.store(rd, 1);
            }
            return Ok(());
        }
        if w & 0x3b200c00 == 0x38000000 {
            // unscaled load/store
            let size = w >> 30;
            let op = w >> 22 & 3;
            if op > 2 || (op == 2 && size != 2) {
                return Err(format!("unsupported unscaled memory 0x{w:08x}"));
            }
            let offset = ((w << 11) as i32) >> 23;
            self.base(0, rn);
            if op == 0 {
                self.load(1, rd);
            }
            self.memory(op != 0, size, op == 2, offset);
            if op != 0 {
                self.store(rd, 1);
            }
            return Ok(());
        }
        if w & 0x3a000000 == 0x28000000 {
            // load/store pair
            let load = w & (1 << 22) != 0;
            let size = if wide { 3 } else { 2 };
            let offset = (((w << 10) as i32) >> 25) << size;
            let mode = w >> 23 & 3;
            let rt2 = w >> 10 & 31;
            self.base(0, rn);
            if mode == 3 {
                self.emit(&[0x48, 0x05]);
                self.disp32(offset);
                self.state_store((rn * 8) as i32, 0);
            }
            let memory_offset = if mode == 2 { offset } else { 0 };
            if !load {
                self.load(1, rd);
            }
            self.memory(load, size, false, memory_offset);
            if load {
                self.store(rd, 1);
            }
            if !load {
                self.load(1, rt2);
            }
            self.memory(load, size, false, memory_offset + (1 << size));
            if load {
                self.store(rt2, 1);
            }
            if mode == 1 {
                self.emit(&[0x48, 0x05]);
                self.disp32(offset);
                self.state_store((rn * 8) as i32, 0);
            }
            return Ok(());
        }
        Err(format!(
            "unsupported instruction in x86-64 lowering: 0x{w:08x}"
        ))
    }
}

pub(crate) fn lower(
    words: &[u32],
    labels: &[Option<usize>],
    fixups: &[(usize, Label, Branch)],
    data_start: usize,
) -> Result<Vec<u8>, String> {
    lower_dynamic(words, labels, fixups, data_start, &[])
}

pub(crate) fn lower_dynamic(
    words: &[u32],
    labels: &[Option<usize>],
    fixups: &[(usize, Label, Branch)],
    data_start: usize,
    data_fixups: &[(usize, usize, u32)],
) -> Result<Vec<u8>, String> {
    if data_start > words.len() {
        return Err("invalid native data boundary".into());
    }
    let mut lower = Lower {
        code: Vec::new(),
        relocations: Vec::new(),
        stack_failures: Vec::new(),
        globals: Vec::new(),
    };
    lower.prelude();
    let mut offsets = vec![0; words.len() + 1];
    let mut pending: Vec<Option<(usize, &Branch)>> = (0..data_start).map(|_| None).collect();
    let mut globals = vec![None; data_start];
    let mut skip = vec![false; data_start];
    for &(at, slot, reg) in data_fixups {
        if at + 1 >= data_start || reg >= 31 {
            return Err("invalid native global relocation".into());
        }
        globals[at] = Some((slot, reg));
        skip[at + 1] = true;
    }
    for (at, label, branch) in fixups {
        let target = labels
            .get(*label)
            .copied()
            .flatten()
            .ok_or("unresolved function or branch")?;
        if *at >= data_start || target > words.len() {
            return Err("invalid native relocation".into());
        }
        pending[*at] = Some((target, branch));
    }
    for (i, word) in words[..data_start].iter().enumerate() {
        offsets[i] = lower.code.len();
        if let Some((slot, reg)) = globals[i] {
            lower.emit(&[0x48, 0x8d, 0x05]); // lea rax,[rip+global]
            lower.globals.push((lower.code.len(), slot));
            lower.disp32(0);
            lower.store(reg, 0);
        } else if skip[i] {
            continue;
        } else if let Some((target, branch)) = pending[i] {
            lower.branch(branch, target);
        } else {
            lower
                .word(*word)
                .map_err(|e| format!("{e} at instruction {i}"))?;
        }
    }
    // A shared failure block keeps stack checking small and deterministic.
    for at in std::mem::take(&mut lower.stack_failures) {
        lower.patch_here(at);
    }
    lower.imm64(0, 231); // terminate library workers on value-stack exhaustion
    lower.imm64(7, 110);
    lower.emit(&[0x0f, 0x05, 0x0f, 0x0b]);
    while lower.code.len() % 8 != (data_start * 4) % 8 {
        lower.code.push(0);
    }
    for i in data_start..words.len() {
        offsets[i] = lower.code.len();
        lower.emit(&words[i].to_le_bytes());
    }
    offsets[words.len()] = lower.code.len();
    for (at, target) in &lower.relocations {
        let delta = offsets[*target] as i64 - (*at + 4) as i64;
        let delta =
            i32::try_from(delta).map_err(|_| "x86-64 program exceeds relative branch range")?;
        lower.code[*at..*at + 4].copy_from_slice(&delta.to_le_bytes());
    }
    let data = crate::native::elf::dynamic_data_offset(lower.code.len());
    for (at, slot) in &lower.globals {
        let target = data + slot * 8;
        let from = crate::native::elf::TEXT_OFFSET + at + 4;
        let delta = i32::try_from(target as i64 - from as i64)
            .map_err(|_| "x86-64 runtime globals exceed relative address range")?;
        lower.code[*at..*at + 4].copy_from_slice(&delta.to_le_bytes());
    }
    Ok(lower.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_relocations_point_to_native_data_and_unknown_code_is_rejected() {
        let text = lower(
            &[0, 0xd503201f, 0xd4200000, 0xdeadbeef, 0xfeedface],
            &[Some(3)],
            &[(0, 0, Branch::Address(0))],
            3,
        )
        .unwrap();
        let address = text
            .windows(3)
            .position(|w| w == [0x48, 0x8d, 0x05])
            .unwrap();
        let displacement = i32::from_le_bytes(text[address + 3..address + 7].try_into().unwrap());
        let target = (address as i64 + 7 + i64::from(displacement)) as usize;
        assert_eq!(
            &text[target..],
            &[0xef, 0xbe, 0xad, 0xde, 0xce, 0xfa, 0xed, 0xfe]
        );
        assert!(lower(&[0xffffffff], &[], &[], 1)
            .unwrap_err()
            .contains("unsupported instruction"));
        assert!(lower(&[0], &[None], &[(0, 0, Branch::Jump)], 1)
            .unwrap_err()
            .contains("unresolved"));
    }

    #[test]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn native_library_calls_marshal_stack_arguments_floats_and_preserve_state() {
        use std::{fs, os::unix::fs::PermissionsExt, process::Command};
        fn immediate(words: &mut Vec<u32>, reg: u32, value: u64) {
            words.push(0xd2800000 | ((value as u32 & 65535) << 5) | reg);
            for shift in 1..4 {
                let part = (value >> (shift * 16)) as u32 & 65535;
                if part != 0 {
                    words.push(0xf2800000 | shift << 21 | part << 5 | reg);
                }
            }
        }
        fn global(
            words: &mut Vec<u32>,
            fixups: &mut Vec<(usize, usize, u32)>,
            slot: usize,
            reg: u32,
        ) {
            fixups.push((words.len(), slot, reg));
            words.extend([0, 0]);
        }
        fn write_buffer(words: &mut Vec<u32>, globals: &mut Vec<(usize, usize, u32)>) {
            words.push(0xaa0003e2); // x2 = returned snprintf length.
            global(words, globals, 16, 1);
            immediate(words, 0, 1);
            immediate(words, 8, 64);
            words.push(0xd4000001);
        }
        fn check_preserved_arguments(
            words: &mut Vec<u32>,
            fixups: &mut Vec<(usize, usize, Branch)>,
        ) {
            immediate(words, 3, 256);
            for cmp in [0xeb03003f, 0xeb17005f] {
                // cmp x1,x3; cmp x2,x23
                words.push(cmp);
                fixups.push((words.len(), 2, Branch::Cond(1)));
                words.extend([0, 0xd503201f]);
            }
        }
        let mut words = Vec::new();
        let mut globals = Vec::new();
        let mut fixups = Vec::new();
        immediate(&mut words, 22, 77); // Must survive both native calls.
        global(&mut words, &mut globals, 0, 17);
        words.push(0xf9400231); // x17 = imported snprintf.
        global(&mut words, &mut globals, 16, 0);
        immediate(&mut words, 1, 256);
        fixups.push((words.len(), 0, Branch::Address(2)));
        words.extend([0, 0xd503201f]);
        for arg in 3..12 {
            immediate(&mut words, arg, u64::from(arg - 2));
        }
        words.push(0xaa0203f7); // x23 = x2, retained format pointer.
        words.push(0xd100c3ff); // sub sp,sp,#48
        for arg in 6..12 {
            words.push(0xf90003e0 | (arg - 6) << 10 | arg);
        }
        immediate(&mut words, 16, 48);
        words.push(0xfe000611); // Six register arguments plus the outgoing stack block.
        words.push(0x9100c3ff);
        check_preserved_arguments(&mut words, &mut fixups);
        write_buffer(&mut words, &mut globals);

        global(&mut words, &mut globals, 0, 17);
        words.push(0xf9400231);
        immediate(&mut words, 16, 6.25f64.to_bits());
        words.push(0x9e670200); // fmov d0,x16.
        immediate(&mut words, 16, (-1.5f64).to_bits());
        words.push(0x9e670201); // fmov d1,x16.
        global(&mut words, &mut globals, 16, 0);
        immediate(&mut words, 1, 256);
        fixups.push((words.len(), 1, Branch::Address(2)));
        words.extend([0, 0xd503201f]);
        words.push(0xaa0203f7); // x23 = x2.
        immediate(&mut words, 16, 0);
        words.push(0xfe020311); // Three integer and two floating arguments.
        check_preserved_arguments(&mut words, &mut fixups);
        write_buffer(&mut words, &mut globals);
        words.push(0xaa1603e0); // exit with the preserved x22.
        immediate(&mut words, 8, 93);
        words.push(0xd4000001);
        let corrupted = words.len();
        immediate(&mut words, 0, 78);
        immediate(&mut words, 8, 93);
        words.push(0xd4000001);
        let data_start = words.len();
        let mut labels = Vec::new();
        for format in [
            b"%ld,%ld,%ld,%ld,%ld,%ld,%ld,%ld,%ld\n\0".as_slice(),
            b"%.2f %.2f\n\0",
        ] {
            labels.push(Some(words.len()));
            for chunk in format.chunks(4) {
                let mut bytes = [0; 4];
                bytes[..chunk.len()].copy_from_slice(chunk);
                words.push(u32::from_le_bytes(bytes));
            }
        }
        labels.push(Some(corrupted));
        let text = lower_dynamic(&words, &labels, &fixups, data_start, &globals).unwrap();
        let imports = crate::native::ffi::Imports {
            libraries: vec!["libc.so.6".into()],
            symbols: vec![(0, "snprintf".into())],
        };
        let image = crate::native::elf::dynamic_executable(&text, 62, &imports, 4096);
        let path = std::env::temp_dir().join(format!("dunder-native-call-{}", std::process::id()));
        fs::write(&path, image).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let output = Command::new(&path).env_clear().output().unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(output.status.code(), Some(77), "{output:?}");
        assert_eq!(output.stdout, b"1,2,3,4,5,6,7,8,9\n6.25 -1.50\n");
        assert!(output.stderr.is_empty());
    }

    #[test]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn large_recursive_frames_stop_before_corrupting_the_register_bank() {
        use std::{fs, os::unix::fs::PermissionsExt, process::Command};
        let path = std::env::temp_dir().join(format!("dunder-x64-stack-{}", std::process::id()));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let cleanup = Cleanup(path);
        for (depth, expected) in [(2, 3), (2000, 110)] {
            let source = format!(
                r#"
                fn descend(n: i32) -> i32 {{
                    var values: [i64; 8192] = [0; 8192]
                    values[0] = i64(n)
                    if n > 0 {{ return i32(values[0]) + descend(n - 1) }}
                    return i32(values[0])
                }}
                fn main() -> i32 {{ return descend({depth}) }}
            "#
            );
            let image = crate::compile_for(&source, crate::target::Target::X86_64Linux).unwrap();
            fs::write(&cleanup.0, image).unwrap();
            fs::set_permissions(&cleanup.0, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(
                Command::new(&cleanup.0).status().unwrap().code(),
                Some(expected)
            );
        }
    }
}
