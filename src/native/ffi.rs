//! Foreign symbols reachable from this entrypoint, and fixed-signature scalar
//! C calls. Library and slot order follows source order for reproducible images.
use super::{align, Code};
use crate::check::{Expr, Program};
use crate::syntax::Type;
use crate::target::Target;

#[derive(Default)]
pub(crate) struct Imports {
    pub libraries: Vec<String>,
    pub symbols: Vec<(usize, String)>,
}

impl Imports {
    fn add(&mut self, target: Target, library: &str, name: &str) -> Result<usize, String> {
        if [library, name]
            .iter()
            .any(|s| s.is_empty() || s.as_bytes().contains(&0))
        {
            return Err(
                "external library and symbol names must be nonempty and contain no NUL".into(),
            );
        }
        let library = match (library, target) {
            ("dl" | "c", Target::Arm64Macos) => super::image::LIBSYSTEM,
            ("dl", _) => "libdl.so.2",
            ("c", _) => "libc.so.6",
            _ => library,
        };
        let lib = match self.libraries.iter().position(|l| l == library) {
            Some(i) => i,
            None => {
                self.libraries.push(library.to_owned());
                self.libraries.len() - 1
            }
        };
        if let Some(slot) = self
            .symbols
            .iter()
            .position(|(l, s)| *l == lib && s == name)
        {
            return Ok(slot);
        }
        if target != Target::Arm64Macos && self.symbols.iter().any(|(_, s)| s == name) {
            return Err(format!("ELF cannot bind '{name}' separately to multiple libraries; use std.dl to select an individual library"));
        }
        self.symbols.push((lib, name.to_owned()));
        Ok(self.symbols.len() - 1)
    }

    pub(super) fn collect(
        code: &mut Code,
        program: &Program,
        reachable: &[bool],
    ) -> Result<Self, String> {
        let mut imports = Self::default();
        if code.target == Target::Arm64Macos {
            imports.libraries.push(super::image::LIBSYSTEM.to_owned());
        }
        for (i, function) in program.functions.iter().enumerate() {
            if let Some((library, name)) = &function.external {
                if reachable[i] {
                    let slot = imports.add(code.target, library, name)?;
                    code.foreign_slots[i] = Some(slot);
                }
            }
        }
        code.import_count = imports.symbols.len();
        Ok(imports)
    }
}

// C aggregates currently admitted by the checker are homogeneous float records.
// Darwin/AAPCS64 use one vector register per member. SysV classifies each
// eightbyte separately, with aggregates larger than 16 bytes passed in memory.
#[derive(Clone, Copy)]
enum ForeignArg {
    Gp(u32),
    Fp {
        first: u32,
        stride: usize,
        count: usize,
    },
    Stack(usize),
}
struct ForeignFrame {
    locations: Vec<ForeignArg>,
    gp: u32,
    fp: u32,
    stack: usize,
    indirect_result: bool,
}
impl Code<'_> {
    fn ffi_size(&self, ty: Type) -> usize {
        self.types
            .storage_bytes(ty)
            .unwrap_or_else(|| self.types.scalar_size(ty))
    }
    fn fp_shape(&self, ty: Type) -> Option<(usize, usize)> {
        if ty.float() {
            return Some((self.types.scalar_size(ty), 1));
        }
        let (elem, count) = self.types.homogeneous_float(ty)?;
        let size = self.types.scalar_size(elem);
        if self.target == Target::X86_64Linux {
            let bytes = count * size;
            return (bytes <= 16).then(|| (8, bytes.div_ceil(8)));
        }
        Some((size, count))
    }
    fn foreign_frame(&self, params: &[Type], ret: Type) -> ForeignFrame {
        let sysv = self.target == Target::X86_64Linux;
        let indirect_result =
            sysv && self.types.homogeneous_float(ret).is_some() && self.ffi_size(ret) > 16;
        let mut frame = ForeignFrame {
            locations: Vec::with_capacity(params.len()),
            gp: u32::from(indirect_result),
            fp: 0,
            stack: 0,
            indirect_result,
        };
        for &ty in params {
            let mut location = None;
            if let Some((stride, count)) = self.fp_shape(ty) {
                if frame.fp as usize + count <= 8 {
                    location = Some(ForeignArg::Fp {
                        first: frame.fp,
                        stride,
                        count,
                    });
                    frame.fp += count as u32;
                } else if !sysv {
                    frame.fp = 8;
                }
            } else if self.types.homogeneous_float(ty).is_none()
                && frame.gp < if sysv { 6 } else { 8 }
            {
                location = Some(ForeignArg::Gp(frame.gp));
                frame.gp += 1;
            }
            frame.locations.push(location.unwrap_or_else(|| {
                let bytes = self.ffi_size(ty);
                let alignment = if self.target == Target::Arm64Macos {
                    self.types
                        .homogeneous_float(ty)
                        .map_or(bytes, |(elem, _)| self.types.scalar_size(elem))
                } else {
                    8
                };
                let at = align(frame.stack, alignment);
                frame.stack = at
                    + if self.target == Target::Arm64Macos {
                        bytes
                    } else {
                        align(bytes, 8)
                    };
                ForeignArg::Stack(at)
            }));
        }
        frame.stack = align(frame.stack, 16);
        frame
    }

    fn load_piece(&mut self, dst: u32, base: u32, off: usize, bytes: usize) {
        let op = match bytes {
            1 => 0x39400000,
            2 => 0x79400000,
            4 => 0xb9400000,
            8 => 0xf9400000,
            _ => unreachable!(),
        };
        self.emit(op | ((off / bytes) as u32) << 10 | base << 5 | dst);
    }
    fn store_piece(&mut self, src: u32, base: u32, off: usize, bytes: usize) {
        let op = match bytes {
            1 => 0x39000000,
            2 => 0x79000000,
            4 => 0xb9000000,
            8 => 0xf9000000,
            _ => unreachable!(),
        };
        self.emit(op | ((off / bytes) as u32) << 10 | base << 5 | src);
    }
    fn ffi_copy(&mut self, src: u32, dst: u32, bytes: usize) {
        let mut offset = 0;
        while offset < bytes {
            let width = (bytes - offset).min(8);
            self.load_piece(12, src, offset, width);
            self.store_piece(12, dst, offset, width);
            offset += width;
        }
    }

    pub(super) fn foreign_call(
        &mut self,
        target: &Expr,
        args: &[Expr],
        ret: Type,
        storage: Option<usize>,
    ) {
        let params = args.iter().map(|arg| arg.ty).collect::<Vec<_>>();
        let frame = self.foreign_frame(&params, ret);
        let mut size = frame.stack;
        let target_slot = size;
        size += 8;
        let offsets = args
            .iter()
            .map(|arg| {
                let off = size;
                size += align(self.ffi_size(arg.ty), 8);
                off
            })
            .collect::<Vec<_>>();
        size = align(size, 16);
        self.stack_adjust(size, true);
        self.expr(target);
        self.sp_addr(target_slot);
        self.str(0, 9);
        for (arg, &offset) in args.iter().zip(&offsets) {
            self.expr(arg);
            self.sp_addr(offset);
            if self.types.homogeneous_float(arg.ty).is_some() {
                self.ffi_copy(0, 9, self.ffi_size(arg.ty));
            } else {
                self.str(0, 9);
            }
        }
        for ((arg, &offset), location) in args.iter().zip(&offsets).zip(&frame.locations) {
            self.sp_addr(offset);
            let bytes = self.ffi_size(arg.ty);
            match *location {
                ForeignArg::Gp(reg) => self.ldr(reg, 9),
                ForeignArg::Fp {
                    first,
                    stride,
                    count,
                } => {
                    for i in 0..count {
                        let width = (bytes - i * stride).min(stride);
                        self.load_piece(10, 9, i * stride, width);
                        self.bits_to_fp(
                            first + i as u32,
                            10,
                            if width == 8 { Type::F64 } else { Type::F32 },
                        );
                    }
                }
                ForeignArg::Stack(off) => {
                    self.mov(13, 9);
                    self.sp_addr(off);
                    self.ffi_copy(13, 9, bytes);
                }
            }
        }
        if frame.indirect_result {
            self.local_addr(storage.expect("aggregate result storage"));
            self.mov(0, 9);
        }
        self.sp_addr(target_slot);
        self.ldr(17, 9);
        let context_reg = 15;
        if self.has_callbacks {
            self.callback_context(context_reg);
            if self.target != Target::X86_64Linux {
                for reg in 19..=24 {
                    self.store_piece(reg, context_reg, (reg - 19) as usize * 8, 8);
                }
            }
        }
        if self.target == Target::X86_64Linux {
            self.imm(16, frame.stack as u64);
            self.emit(
                (if self.has_callbacks {
                    0xfc000000
                } else {
                    0xfe000000
                }) | frame.fp << 16
                    | frame.gp << 8
                    | 17,
            );
        } else {
            self.emit(0xd63f0220);
            if self.has_callbacks {
                self.callback_context(context_reg);
                for reg in 19..=24 {
                    self.load_piece(reg, context_reg, (reg - 19) as usize * 8, 8);
                }
            }
        }
        self.stack_adjust(size, false);
        if let Some(storage) = storage {
            self.local_addr(storage);
            if !frame.indirect_result {
                let (stride, count) = self.fp_shape(ret).unwrap();
                for i in 0..count {
                    let width = (self.ffi_size(ret) - i * stride).min(stride);
                    self.fp_to_bits(10, i as u32, if width == 8 { Type::F64 } else { Type::F32 });
                    self.store_piece(10, 9, i * stride, width);
                }
            }
            self.mov(0, 9);
        } else if ret.float() {
            self.fp_to_bits(0, 0, ret);
        } else if ret == Type::I32 || ret == Type::Rune || self.types.scalar_enum(ret) {
            self.emit(0x93407c00);
        } else if self.types.scalar_size(ret) < 8 {
            let bits = self.types.scalar_size(ret) as u32 * 8;
            self.emit(0xd3400000 | (bits - 1) << 10);
        }
    }
}

impl Code<'_> {
    pub(super) fn prepare_callbacks(&mut self, program: &Program, reachable: &[bool]) {
        let mut used = vec![false; program.functions.len()];
        for (function, reachable) in program.functions.iter().zip(reachable) {
            if *reachable {
                super::reachable::visit_body(&function.body, false, &mut |expr, _| {
                    if let crate::check::ExprKind::Callback(id) = expr.kind {
                        used[id] = true;
                    }
                });
            }
        }
        self.has_callbacks = used.iter().any(|&v| v);
        self.callbacks = used
            .into_iter()
            .map(|used| used.then(|| self.label()))
            .collect();
    }

    // First two private globals belong to the graphics libraries. The callback
    // context starts two words later and is valid on the originating thread.
    fn callback_context(&mut self, reg: u32) {
        self.data_slot(self.import_count + 2, reg);
    }
    pub(super) fn callback_entries(&mut self, program: &Program) {
        for (i, function) in program.functions.iter().enumerate() {
            let Some(label) = self.callbacks[i] else {
                continue;
            };
            self.mark(label);
            let x86 = self.target == Target::X86_64Linux;
            if x86 {
                // Enter SysV with native registers, reacquire Dunder's virtual
                // register bank and suspended value stack, capture caller SP in x14.
                self.emit(0xfd000000 | (self.import_count + 2) as u32);
            } else {
                self.emit(0x910003ee); // mov x14,sp (incoming stack arguments)
            }
            self.prologue(0);
            if !x86 {
                self.callback_context(15);
                for reg in 19..=24 {
                    self.push(reg);
                    self.load_piece(reg, 15, (reg - 19) as usize * 8, 8);
                }
            }
            let params = &function.params;
            let incoming = self.foreign_frame(params, function.ret);
            let (outgoing, stack) = super::arguments(params, self.types);
            let scratch = align(params.len() * 8, 16);
            self.stack_adjust(stack + scratch, true);
            for (i, (&ty, location)) in params.iter().zip(incoming.locations).enumerate() {
                match location {
                    ForeignArg::Gp(reg) => self.mov(10, reg),
                    ForeignArg::Fp { first, .. } => self.fp_to_bits(10, first, ty),
                    ForeignArg::Stack(off) => {
                        self.imm(10, off as u64);
                        self.add(13, 14, 10);
                        self.load_piece(10, 13, 0, self.ffi_size(ty));
                    }
                }
                // Normalise unspecified upper bits on ingress to Dunder's full
                // scalar slots, including negative signed stack arguments.
                if !ty.float() && self.ffi_size(ty) < 8 {
                    let bits = self.ffi_size(ty) as u32 * 8;
                    let op = if matches!(ty, Type::I32 | Type::Rune) {
                        0x93400000
                    } else {
                        0xd3400000
                    };
                    self.emit(op | (bits - 1) << 10 | 10 << 5 | 10);
                }
                self.sp_addr(stack + i * 8);
                self.str(10, 9);
            }
            for (i, location) in outgoing.into_iter().enumerate() {
                self.sp_addr(stack + i * 8);
                match location {
                    super::Argument::General(reg) => self.ldr(reg, 9),
                    super::Argument::Float(reg) => {
                        self.ldr(10, 9);
                        self.bits_to_fp(reg, 10, params[i]);
                    }
                    super::Argument::Stack(offset) => {
                        self.ldr(10, 9);
                        self.sp_addr(offset);
                        self.typed_store_reg(params[i], 9, 10);
                    }
                }
            }
            self.branch(self.functions[i], super::Branch::Call);
            self.stack_adjust(stack + scratch, false);
            if x86 {
                self.emit(0x910003bf);
                self.emit(0xa8c17bfd);
                self.emit(0xfb000000); // transfer scalar return and restore SysV state
            } else {
                self.callback_context(15);
                for reg in (19..=24).rev() {
                    self.store_piece(reg, 15, (reg - 19) as usize * 8, 8);
                    self.pop(reg);
                }
                self.epilogue();
            }
        }
    }
}
