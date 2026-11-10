//! Immutable UTF-8 values: one-word handles to [u64 byte_length, UTF-8 bytes].
//! Literals are immortal; dynamic values use non-atomic reference counting and
//! reusable size-class blocks in a bounded arena. Immutable leaves cannot cycle.
//! x19/x20 are reserved across all generated Dunder code for its cursor/end.
use super::{Branch, Code, Label};
use crate::{
    check::{Expr, StringOp},
    syntax::Type,
};

const ARENA_BYTES: u64 = 16 * 1024 * 1024;
const MAPPING_BYTES: u64 = ARENA_BYTES + 4096;
const BIN_BYTES: u32 = 512;
// Dynamic block: [class + 1, free-list next, references, byte length, bytes...].
// Handles address byte length, so public string layout remains [length, bytes].
// The allocator never moves live strings; only the final release recycles them.

#[derive(Default)]
pub(super) struct Runtime {
    pub alloc: Label,
    pub try_alloc: Label,
    pub retain: Label,
    pub release: Label,
    pub compare: Label,
    pub concat: Label,
    pub slice: Label,
    pub decode: Label,
    pub rune_at: Label,
    pub rune_len: Label,
    pub from_int: Label,
    pub from_rune: Label,
    pub to_rune: Label,
    pub print: Label,
    pub print_rune: Label,
    pub exhausted: Label,
    pub unicode_error: Label,
    pub(super) literals: Vec<(String, Label)>,
}
impl Runtime {
    pub fn new(c: &mut Code<'_>) -> Self {
        Self {
            alloc: c.label(),
            try_alloc: c.label(),
            retain: c.label(),
            release: c.label(),
            compare: c.label(),
            concat: c.label(),
            slice: c.label(),
            decode: c.label(),
            rune_at: c.label(),
            rune_len: c.label(),
            from_int: c.label(),
            from_rune: c.label(),
            to_rune: c.label(),
            print: c.label(),
            print_rune: c.label(),
            exhausted: c.label(),
            unicode_error: c.label(),
            literals: Vec::new(),
        }
    }
}

impl Code<'_> {
    pub(super) fn add(&mut self, dst: u32, a: u32, b: u32) {
        self.emit(0x8b000000 | b << 16 | a << 5 | dst);
    }
    pub(super) fn sub(&mut self, dst: u32, a: u32, b: u32) {
        self.emit(0xcb000000 | b << 16 | a << 5 | dst);
    }
    pub(super) fn add_imm(&mut self, dst: u32, src: u32, n: u32) {
        self.emit(0x91000000 | n << 10 | src << 5 | dst);
    }
    pub(super) fn sub_imm(&mut self, dst: u32, src: u32, n: u32) {
        self.emit(0xd1000000 | n << 10 | src << 5 | dst);
    }
    pub(super) fn ldr(&mut self, dst: u32, src: u32) {
        self.emit(0xf9400000 | src << 5 | dst);
    }
    pub(super) fn str(&mut self, src: u32, dst: u32) {
        self.emit(0xf9000000 | dst << 5 | src);
    }
    pub(super) fn ldrb(&mut self, dst: u32, src: u32) {
        self.emit(0x39400000 | src << 5 | dst);
    }
    pub(super) fn strb(&mut self, src: u32, dst: u32) {
        self.emit(0x39000000 | dst << 5 | src);
    }
    pub(super) fn and(&mut self, dst: u32, a: u32, b: u32) {
        self.emit(0x8a000000 | b << 16 | a << 5 | dst);
    }
    pub(super) fn or(&mut self, dst: u32, a: u32, b: u32) {
        self.emit(0xaa000000 | b << 16 | a << 5 | dst);
    }
    pub(super) fn lsr(&mut self, dst: u32, src: u32, n: u32) {
        self.emit(0xd340fc00 | n << 16 | src << 5 | dst);
    }
    pub(super) fn lsl(&mut self, dst: u32, src: u32, n: u32) {
        self.emit(0xd3400000 | (64 - n) << 16 | (63 - n) << 10 | src << 5 | dst);
    }
    pub(super) fn ret(&mut self) {
        self.emit(0xd65f03c0);
    }

    pub(super) fn string_literal(&mut self, s: &str) -> Label {
        if let Some((_, label)) = self.strings.literals.iter().find(|(text, _)| text == s) {
            if let Some(trace) = &mut self.literal_trace {
                trace.push(*label);
            }
            return *label;
        }
        let label = self.label();
        if let Some(trace) = &mut self.literal_trace {
            trace.push(label);
        }
        self.strings.literals.push((s.into(), label));
        label
    }
    pub(super) fn string_literals(&mut self) {
        for (text, label) in std::mem::take(&mut self.strings.literals) {
            if !self.words.len().is_multiple_of(2) {
                self.emit(0);
            }
            self.emit(0); // zero reference count means an immortal literal
            self.emit(0);
            self.mark(label);
            let len = text.len() as u64;
            self.emit(len as u32);
            self.emit((len >> 32) as u32);
            let mut bytes = text.into_bytes();
            bytes.resize(super::align(bytes.len() + 1, 8), 0); // immutable C literal terminator
            for word in bytes.as_chunks::<4>().0 {
                self.emit(u32::from_le_bytes(*word));
            }
        }
    }
    pub(super) fn string_expr(&mut self, op: StringOp, args: &[Expr]) {
        self.expr(&args[0]);
        if args.len() > 1 {
            self.push(0);
            self.expr(&args[1]);
            self.pop(1);
        }
        match op {
            StringOp::Len => self.ldr(0, 0),
            StringOp::RuneLen => self.branch(self.strings.rune_len, Branch::Call),
            StringOp::RuneAt => self.branch(self.strings.rune_at, Branch::Call),
            StringOp::Slice => self.branch(self.strings.slice, Branch::Call),
            StringOp::ByteAt => {
                self.ldr(2, 1);
                self.cmp(0, 2);
                self.branch(self.bounds, Branch::Cond(2));
                self.add(0, 1, 0);
                self.add_imm(0, 0, 8);
                self.ldrb(0, 0);
            }
            StringOp::ToRune => self.branch(self.strings.to_rune, Branch::Call),
            StringOp::ToString => match args[0].ty {
                Type::F32 | Type::F64 => self.float_to_string(args[0].ty),
                Type::String => {}
                Type::Rune => self.branch(self.strings.from_rune, Branch::Call),
                Type::Bool => {
                    let falsity = self.label();
                    let end = self.label();
                    self.branch(falsity, Branch::Zero(0));
                    let yes = self.string_literal("true");
                    let no = self.string_literal("false");
                    self.branch(yes, Branch::Address(0));
                    self.branch(end, Branch::Jump);
                    self.mark(falsity);
                    self.branch(no, Branch::Address(0));
                    self.mark(end);
                }
                _ => {
                    self.imm(8, u64::from(args[0].ty.unsigned()));
                    self.branch(self.strings.from_int, Branch::Call);
                }
            },
        }
    }

    // Clobbers byte, source, destination, count. Copies exactly count bytes.
    pub(super) fn copy_bytes(&mut self, dst: u32, src: u32, count: u32, byte: u32) {
        let end = self.label();
        let again = self.label();
        let tail = self.label();
        let wide = self.label();
        self.mark(wide);
        self.emit(0xf100201f | count << 5); // cmp count,#8
        self.branch(tail, Branch::Cond(3));
        self.ldr(byte, src);
        self.str(byte, dst);
        self.add_imm(src, src, 8);
        self.add_imm(dst, dst, 8);
        self.sub_imm(count, count, 8);
        self.branch(wide, Branch::Jump);
        self.mark(tail);
        self.branch(end, Branch::Zero(count));
        self.mark(again);
        self.ldrb(byte, src);
        self.strb(byte, dst);
        self.add_imm(src, src, 1);
        self.add_imm(dst, dst, 1);
        self.sub_imm(count, count, 1);
        self.branch(again, Branch::Nonzero(count));
        self.mark(end);
    }
    pub(super) fn string_runtime(&mut self) {
        self.mark(self.strings.alloc);
        self.prologue(0);
        self.branch(self.strings.try_alloc, Branch::Call);
        self.branch(self.strings.exhausted, Branch::Zero(0));
        self.epilogue();
        self.mark(self.strings.try_alloc);
        let mapped = self.label();
        let map_failed = self.label();
        let exhausted = self.label();
        self.imm(1, ARENA_BYTES);
        self.cmp(0, 1);
        self.branch(exhausted, Branch::Cond(8));
        self.branch(mapped, Branch::Nonzero(19));
        self.push(0);
        self.imm(0, 0);
        self.imm(1, MAPPING_BYTES);
        self.imm(2, 3); // read/write
        self.imm(
            3,
            if self.target == crate::target::Target::Arm64Macos {
                0x1002
            } else {
                0x22
            },
        );
        self.imm(4, u64::MAX);
        self.imm(5, 0); // MAP_ANON | MAP_PRIVATE
        self.syscall(197); // anonymous private mmap
        self.branch(map_failed, Branch::Cond(2));
        self.mov(19, 0);
        self.imm(1, MAPPING_BYTES);
        self.add(20, 19, 1);
        self.add_imm(19, 19, BIN_BYTES);
        self.pop(0);
        self.mark(mapped);
        // Capacity excludes the 32-byte block header. Even an exact 16 MiB
        // payload fits; the extra mapping page pays for bins and headers.
        self.imm(1, 16);
        self.imm(2, 0);
        let classify = self.label();
        let classified = self.label();
        let fresh = self.label();
        let initialize = self.label();
        self.mark(classify);
        self.cmp(1, 0);
        self.branch(classified, Branch::Cond(2));
        self.lsl(1, 1, 1);
        self.add_imm(2, 2, 1);
        self.branch(classify, Branch::Jump);
        self.mark(classified);
        self.imm(3, MAPPING_BYTES);
        self.sub(4, 20, 3);
        self.lsl(3, 2, 3);
        self.add(4, 4, 3);
        self.ldr(5, 4);
        self.branch(fresh, Branch::Zero(5));
        self.add_imm(3, 5, 8);
        self.ldr(3, 3);
        self.str(3, 4);
        self.branch(initialize, Branch::Jump);
        self.mark(fresh);
        self.add_imm(1, 1, 32);
        self.add(3, 19, 1);
        self.cmp(3, 20);
        self.branch(exhausted, Branch::Cond(8));
        self.mov(5, 19);
        self.mov(19, 3);
        self.mark(initialize);
        self.add_imm(2, 2, 1);
        self.str(2, 5); // allocated class + 1; zero denotes a free block
        self.add_imm(3, 5, 16);
        self.imm(1, 1);
        self.str(1, 3);
        self.add_imm(5, 5, 24);
        self.str(0, 5);
        self.mov(0, 5);
        self.ret();
        self.mark(map_failed);
        self.pop(0);
        self.mark(exhausted);
        self.imm(0, 0);
        self.ret();

        // Both ownership helpers preserve x0 and every other integer register.
        // This permits use while walking inline records and array elements.
        self.mark(self.strings.retain);
        let retained = self.label();
        self.push(9);
        self.push(10);
        self.branch(retained, Branch::Zero(0));
        self.sub_imm(9, 0, 8);
        self.ldr(10, 9);
        self.branch(retained, Branch::Zero(10));
        // MAX is the freed sentinel; MAX-1 cannot be incremented safely.
        self.push(11);
        self.imm(11, u64::MAX - 1);
        self.cmp(10, 11);
        self.branch(self.strings.exhausted, Branch::Cond(2));
        self.pop(11);
        self.add_imm(10, 10, 1);
        self.str(10, 9);
        self.mark(retained);
        self.pop(10);
        self.pop(9);
        self.ret();

        self.mark(self.strings.release);
        let released = self.label();
        let still_live = self.label();
        for register in 9..=13 {
            self.push(register);
        }
        self.branch(released, Branch::Zero(0));
        self.sub_imm(9, 0, 8);
        self.ldr(10, 9);
        self.branch(released, Branch::Zero(10));
        self.imm(11, u64::MAX);
        self.cmp(10, 11);
        self.branch(self.strings.exhausted, Branch::Cond(0));
        self.sub_imm(10, 10, 1);
        self.branch(still_live, Branch::Nonzero(10));
        self.str(11, 9); // freed handles must not look like immortal literals
        self.sub_imm(9, 0, 24);
        self.ldr(10, 9);
        self.branch(self.strings.exhausted, Branch::Zero(10));
        self.imm(11, 21);
        self.cmp(10, 11);
        self.branch(self.strings.exhausted, Branch::Cond(8));
        self.imm(11, 0);
        self.str(11, 9);
        self.sub_imm(10, 10, 1);
        self.lsl(10, 10, 3);
        self.imm(11, MAPPING_BYTES);
        self.sub(12, 20, 11);
        self.add(12, 12, 10);
        self.ldr(13, 12);
        self.add_imm(11, 9, 8);
        self.str(13, 11);
        self.str(9, 12);
        self.branch(released, Branch::Jump);
        self.mark(still_live);
        self.str(10, 9);
        self.mark(released);
        for register in (9..=13).rev() {
            self.pop(register);
        }
        self.ret();

        self.mark(self.strings.compare); // left x1, right x0; result -1/0/1
        self.ldr(2, 1);
        self.ldr(3, 0);
        self.add_imm(4, 1, 8);
        self.add_imm(5, 0, 8);
        self.imm(6, 0);
        let head = self.label();
        let left_end = self.label();
        let equal = self.label();
        let less = self.label();
        let greater = self.label();
        self.mark(head);
        self.cmp(6, 2);
        self.branch(left_end, Branch::Cond(0));
        self.cmp(6, 3);
        self.branch(greater, Branch::Cond(0));
        self.ldrb(7, 4);
        self.ldrb(8, 5);
        self.cmp(7, 8);
        self.branch(less, Branch::Cond(3));
        self.branch(greater, Branch::Cond(8));
        self.add_imm(4, 4, 1);
        self.add_imm(5, 5, 1);
        self.add_imm(6, 6, 1);
        self.branch(head, Branch::Jump);
        self.mark(left_end);
        self.cmp(6, 3);
        self.branch(equal, Branch::Cond(0));
        self.mark(less);
        self.imm(0, u64::MAX);
        self.ret();
        self.mark(greater);
        self.imm(0, 1);
        self.ret();
        self.mark(equal);
        self.imm(0, 0);
        self.ret();

        self.mark(self.strings.concat);
        self.prologue(0);
        self.push(1);
        self.push(0);
        self.ldr(2, 1);
        self.ldr(3, 0);
        self.add(0, 2, 3);
        self.branch(self.strings.alloc, Branch::Call);
        self.pop(2);
        self.pop(1);
        self.mov(8, 0);
        self.ldr(3, 1);
        self.ldr(4, 2);
        self.add_imm(5, 0, 8);
        self.add_imm(6, 1, 8);
        self.copy_bytes(5, 6, 3, 7);
        self.add_imm(6, 2, 8);
        self.copy_bytes(5, 6, 4, 7);
        self.mov(0, 8);
        self.epilogue();

        self.mark(self.strings.slice); // base x1, packed i32 bounds x0
        self.prologue(0);
        self.emit(0x93407c02);
        self.emit(0x9360fc03); // signed start x2/end x3
        self.ldr(8, 1);
        self.cmp(2, 3);
        self.branch(self.bounds, Branch::Cond(8));
        self.cmp(3, 8);
        self.branch(self.bounds, Branch::Cond(8));
        for index in [2, 3] {
            let valid = self.label();
            self.cmp(index, 8);
            self.branch(valid, Branch::Cond(0));
            self.add(4, 1, index);
            self.add_imm(4, 4, 8);
            self.ldrb(5, 4);
            self.imm(6, 0xc0);
            self.and(5, 5, 6);
            self.imm(6, 0x80);
            self.cmp(5, 6);
            self.branch(self.strings.unicode_error, Branch::Cond(0));
            self.mark(valid);
        }
        self.push(1);
        self.push(2);
        self.push(3);
        self.sub(0, 3, 2);
        self.branch(self.strings.alloc, Branch::Call);
        self.pop(3);
        self.pop(2);
        self.pop(1);
        self.mov(8, 0);
        self.sub(3, 3, 2);
        self.add(6, 1, 2);
        self.add_imm(6, 6, 8);
        self.add_imm(5, 0, 8);
        self.copy_bytes(5, 6, 3, 4);
        self.mov(0, 8);
        self.epilogue();

        // Input points into validated UTF-8 at a scalar boundary. Output rune
        // x0 and byte width x1; clobbers x0..x5 only, allocates nothing.
        self.mark(self.strings.decode);
        self.mov(3, 0);
        self.ldrb(2, 3);
        self.imm(1, 1);
        self.imm(4, 128);
        self.cmp(2, 4);
        let ascii = self.label();
        let two = self.label();
        let three = self.label();
        let decode = self.label();
        self.branch(ascii, Branch::Cond(3));
        self.imm(4, 224);
        self.cmp(2, 4);
        self.branch(two, Branch::Cond(3));
        self.imm(4, 240);
        self.cmp(2, 4);
        self.branch(three, Branch::Cond(3));
        self.imm(1, 4);
        self.imm(4, 7);
        self.branch(decode, Branch::Jump);
        self.mark(two);
        self.imm(1, 2);
        self.imm(4, 31);
        self.branch(decode, Branch::Jump);
        self.mark(three);
        self.imm(1, 3);
        self.imm(4, 15);
        self.mark(decode);
        self.and(0, 2, 4);
        self.imm(5, 1);
        self.add_imm(3, 3, 1);
        let continuation = self.label();
        self.mark(continuation);
        self.ldrb(2, 3);
        self.imm(4, 63);
        self.and(2, 2, 4);
        self.lsl(0, 0, 6);
        self.or(0, 0, 2);
        self.add_imm(3, 3, 1);
        self.add_imm(5, 5, 1);
        self.cmp(5, 1);
        self.branch(continuation, Branch::Cond(3));
        self.ret();
        self.mark(ascii);
        self.mov(0, 2);
        self.ret();

        self.mark(self.strings.rune_len);
        self.prologue(0);
        self.ldr(7, 0);
        self.add_imm(6, 0, 8);
        self.add(7, 6, 7);
        self.imm(8, 0);
        let again = self.label();
        let end = self.label();
        self.mark(again);
        self.cmp(6, 7);
        self.branch(end, Branch::Cond(2));
        self.mov(0, 6);
        self.branch(self.strings.decode, Branch::Call);
        self.add(6, 6, 1);
        self.add_imm(8, 8, 1);
        self.branch(again, Branch::Jump);
        self.mark(end);
        self.mov(0, 8);
        self.epilogue();

        self.mark(self.strings.rune_at);
        self.prologue(0);
        self.mov(6, 0);
        self.ldr(8, 1);
        self.add_imm(7, 1, 8);
        self.add(8, 7, 8);
        let again = self.label();
        let end = self.label();
        self.mark(again);
        self.cmp(7, 8);
        self.branch(self.bounds, Branch::Cond(2));
        self.mov(0, 7);
        self.branch(self.strings.decode, Branch::Call);
        self.branch(end, Branch::Zero(6));
        self.sub_imm(6, 6, 1);
        self.add(7, 7, 1);
        self.branch(again, Branch::Jump);
        self.mark(end);
        self.epilogue();

        self.mark(self.strings.to_rune);
        self.imm(1, 0x10ffff);
        self.cmp(0, 1);
        self.branch(self.strings.unicode_error, Branch::Cond(8));
        let valid = self.label();
        self.imm(1, 0xd800);
        self.cmp(0, 1);
        self.branch(valid, Branch::Cond(3));
        self.imm(1, 0xdfff);
        self.cmp(0, 1);
        self.branch(self.strings.unicode_error, Branch::Cond(9));
        self.mark(valid);
        self.ret();

        self.mark(self.strings.from_rune);
        self.prologue(0);
        let width_set = self.label();
        for (limit, width) in [(128, 1), (2048, 2), (65536, 3)] {
            self.imm(1, width);
            self.imm(2, limit);
            self.cmp(0, 2);
            self.branch(width_set, Branch::Cond(3));
        }
        self.imm(1, 4);
        self.mark(width_set);
        self.push(0);
        self.push(1);
        self.mov(0, 1);
        self.branch(self.strings.alloc, Branch::Call);
        self.pop(1);
        self.pop(2);
        self.add_imm(3, 0, 8);
        let encoded = self.label();
        for width in 1..=4 {
            let next = self.label();
            self.imm(4, width);
            self.cmp(1, 4);
            self.branch(next, Branch::Cond(1));
            if width == 1 {
                self.strb(2, 3);
            } else {
                for byte in 0..width {
                    let shift = 6 * (width - byte - 1) as u32;
                    if shift > 0 {
                        self.lsr(4, 2, shift);
                    } else {
                        self.mov(4, 2);
                    }
                    if byte > 0 {
                        self.imm(5, 63);
                        self.and(4, 4, 5);
                    }
                    self.imm(
                        5,
                        if byte > 0 {
                            0x80
                        } else {
                            match width {
                                2 => 0xc0,
                                3 => 0xe0,
                                _ => 0xf0,
                            }
                        },
                    );
                    self.or(4, 4, 5);
                    self.strb(4, 3);
                    self.add_imm(3, 3, 1);
                }
            }
            self.branch(encoded, Branch::Jump);
            self.mark(next);
        }
        self.mark(encoded);
        self.epilogue();

        self.mark(self.strings.from_int);
        self.prologue(64);
        self.add_imm(1, 31, 64); // add x1,sp,#64
        self.imm(2, 0);
        self.mov(3, 0);
        self.imm(4, 10);
        self.imm(7, 0);
        let magnitude = self.label();
        self.branch(magnitude, Branch::Nonzero(8));
        self.cmp(3, 31);
        self.branch(magnitude, Branch::Cond(10));
        self.imm(7, 1);
        self.sub(3, 31, 3);
        self.mark(magnitude);
        let digit = self.label();
        self.mark(digit);
        self.emit(0x9ac40865);
        self.emit(0x9b048ca6);
        self.add_imm(6, 6, 48);
        self.sub_imm(1, 1, 1);
        self.strb(6, 1);
        self.add_imm(2, 2, 1);
        self.mov(3, 5);
        self.branch(digit, Branch::Nonzero(3));
        let signed = self.label();
        self.branch(signed, Branch::Zero(7));
        self.sub_imm(1, 1, 1);
        self.imm(9, 45);
        self.strb(9, 1);
        self.add_imm(2, 2, 1);
        self.mark(signed);
        self.push(1);
        self.push(2);
        self.mov(0, 2);
        self.branch(self.strings.alloc, Branch::Call);
        self.pop(2);
        self.pop(1);
        self.add_imm(3, 0, 8);
        self.copy_bytes(3, 1, 2, 4);
        self.epilogue();

        self.mark(self.strings.print);
        self.prologue(16);
        self.ldr(2, 0);
        self.add_imm(1, 0, 8);
        let newline = self.label();
        self.branch(newline, Branch::Zero(2));
        self.branch(self.write, Branch::Call);
        self.mark(newline);
        self.imm(9, 10);
        self.emit(0x390003e9);
        self.add_imm(1, 31, 0);
        self.imm(2, 1);
        self.branch(self.write, Branch::Call);
        self.epilogue();
        self.mark(self.strings.print_rune);
        self.prologue(0);
        self.branch(self.strings.from_rune, Branch::Call);
        self.push(0);
        self.branch(self.strings.print, Branch::Call);
        self.pop(0);
        self.branch(self.strings.release, Branch::Call);
        self.epilogue();

        for (label, status) in [
            (self.strings.exhausted, 105),
            (self.strings.unicode_error, 106),
        ] {
            self.mark(label);
            self.imm(0, status);
            self.syscall(1);
            self.emit(0xd4200000);
        }
    }
}
