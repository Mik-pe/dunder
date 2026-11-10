//! Direct typed-tree to AArch64 emission, ELF/Mach-O layout and ad hoc signing.
//! No assembler, linker, interpreter or external signer is invoked.
use crate::{
    check::{Expr, ExprKind, Program, Stmt},
    syntax::{Type, Types},
    target::Target,
};
mod cache;
mod collections;
pub use cache::{CacheStats, Incremental};
pub(crate) mod elf;
mod enums;
pub(crate) mod ffi;
mod fingerprint;
mod floats;
mod regions;
use fingerprint::Fingerprints;
mod image;
pub(crate) mod optimize;
mod reachable;
mod strings;
mod sys;
mod unsigned;

pub(crate) type Label = usize;
pub(crate) enum Branch {
    Jump,
    Call,
    Cond(u32),
    Zero(u32),
    Nonzero(u32),
    Address(u32),
}
struct Code<'a> {
    target: Target,
    data_start: usize,
    types: &'a Types,
    locals: optimize::LocalPlan,
    leaves: Vec<Option<&'a Expr>>,
    words: Vec<u32>,
    labels: Vec<Option<usize>>,
    fixups: Vec<(usize, Label, Branch)>,
    functions: Vec<Label>,
    print_int: Label,
    print_bool: Label,
    write: Label,
    overflow: Label,
    divide_zero: Label,
    io_error: Label,
    bounds: Label,
    assertion: Label,
    loops: Vec<(Label, Label)>,
    strings: strings::Runtime,
    literal_trace: Option<Vec<Label>>,
    float_formatter: Option<Label>,
    float_conversion: Label,
    return_slot: Option<usize>,
    owned_storage: Vec<(usize, Type)>,
    sys: sys::Runtime,
    sys_types: Option<crate::check::SysTypes>,
    invalid_enum: Label,
    collections: collections::Runtime,
    text_offset: usize,
    data_fixups: Vec<(usize, usize, u32)>,
    static_slots: std::collections::BTreeMap<(usize, usize), usize>,
    foreign_slots: Vec<Option<usize>>,
    callbacks: Vec<Option<Label>>,
    has_callbacks: bool,
    import_count: usize,
}
impl<'a> Code<'a> {
    fn new(functions: usize, types: &'a Types) -> Self {
        let mut c = Self {
            target: Target::Arm64Macos,
            data_start: 0,
            types,
            locals: optimize::LocalPlan::default(),
            leaves: Vec::new(),
            words: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
            functions: Vec::new(),
            print_int: 0,
            print_bool: 0,
            write: 0,
            overflow: 0,
            divide_zero: 0,
            io_error: 0,
            bounds: 0,
            assertion: 0,
            loops: Vec::new(),
            strings: strings::Runtime::default(),
            literal_trace: None,
            float_formatter: None,
            float_conversion: 0,
            return_slot: None,
            owned_storage: Vec::new(),
            sys: sys::Runtime::default(),
            sys_types: None,
            invalid_enum: 0,
            collections: collections::Runtime::default(),
            text_offset: 432,
            data_fixups: Vec::new(),
            static_slots: Default::default(),
            foreign_slots: vec![None; functions],
            callbacks: Vec::new(),
            has_callbacks: false,
            import_count: 0,
        };
        c.functions = (0..functions).map(|_| c.label()).collect();
        c.print_int = c.label();
        c.print_bool = c.label();
        c.write = c.label();
        c.overflow = c.label();
        c.divide_zero = c.label();
        c.io_error = c.label();
        c.bounds = c.label();
        c.assertion = c.label();
        c.strings = strings::Runtime::new(&mut c);
        c.float_conversion = c.label();
        c.sys = sys::Runtime::new(&mut c);
        c.invalid_enum = c.label();
        c.collections = collections::Runtime::new(&mut c);
        c
    }
    fn label(&mut self) -> Label {
        self.labels.push(None);
        self.labels.len() - 1
    }
    fn data_slot(&mut self, slot: usize, reg: u32) {
        self.data_fixups.push((self.words.len(), slot, reg));
        self.emit(0);
        self.emit(0);
    }
    fn mark(&mut self, l: Label) {
        self.locals.last_read = None; // incoming branches may carry different values
        assert!(self.labels[l].is_none());
        self.labels[l] = Some(self.words.len());
    }
    fn emit(&mut self, word: u32) {
        self.words.push(word);
    }
    fn branch(&mut self, label: Label, kind: Branch) {
        let wide = matches!(
            kind,
            Branch::Cond(_) | Branch::Zero(_) | Branch::Nonzero(_) | Branch::Address(_)
        );
        self.fixups.push((self.words.len(), label, kind));
        self.emit(0);
        if wide {
            self.emit(0xd503201f);
        }
    }
    fn mov(&mut self, dst: u32, src: u32) {
        if dst != src {
            self.emit(0xaa0003e0 | src << 16 | dst);
        }
    }
    fn imm(&mut self, reg: u32, value: u64) {
        self.emit(0xd2800000 | ((value as u32 & 0xffff) << 5) | reg);
        for part in 1..4 {
            let bits = ((value >> (part * 16)) & 0xffff) as u32;
            if bits != 0 {
                self.emit(0xf2800000 | part << 21 | bits << 5 | reg);
            }
        }
    }
    fn cmp(&mut self, a: u32, b: u32) {
        self.emit(0xeb00001f | b << 16 | a << 5);
    }
    fn push(&mut self, reg: u32) {
        self.emit(0xa9bf7fe0 | reg);
    } // stp reg,xzr,[sp,#-16]!
    fn pop(&mut self, reg: u32) {
        self.emit(0xa8c17fe0 | reg);
    }
    fn local_addr(&mut self, slot: usize) {
        let offset = (slot + 1) * 8;
        if offset <= 4095 {
            self.emit(0xd10003a9 | (offset as u32) << 10);
            return;
        }
        self.imm(9, (slot as u64 + 1) * 8);
        self.emit(0xcb0903a9);
    } // sub x9,x29,x9
    fn store(&mut self, slot: usize, reg: u32) {
        self.locals.write_slot(slot);
        if let Some(home) = self.locals.register(slot) {
            self.mov(home, reg);
            return;
        }
        if slot < 32 {
            self.emit(0xf80003a0 | ((-(slot as i32 + 1) * 8) as u32 & 511) << 12 | reg);
            return;
        }
        self.local_addr(slot);
        self.emit(0xf9000120 | reg);
    }
    fn load(&mut self, slot: usize) {
        self.load_reg(slot, 0);
    }
    fn load_reg(&mut self, slot: usize, reg: u32) {
        if let Some(home) = self.locals.register(slot) {
            self.mov(reg, home);
            return;
        }
        if slot < 32 {
            self.emit(0xf84003a0 | ((-(slot as i32 + 1) * 8) as u32 & 511) << 12 | reg);
            return;
        }
        self.local_addr(slot);
        self.emit(0xf9400120 | reg);
    }
    fn prologue(&mut self, size: usize) {
        self.emit(0xa9bf7bfd);
        self.emit(0x910003fd);
        self.stack_adjust(align(size, 16), true);
    }
    fn stack_adjust(&mut self, size: usize, allocate: bool) {
        let mut left = size;
        while left > 0 {
            let part = left.min(4080);
            self.emit((if allocate { 0xd10003ff } else { 0x910003ff }) | (part as u32) << 10);
            left -= part;
        }
    }
    fn sp_addr(&mut self, offset: usize) {
        self.emit(0x910003e9); // mov x9,sp
        self.imm(11, offset as u64);
        self.emit(0x8b0b0129); // add x9,x9,x11
    }
    fn load_stack_param(&mut self, offset: usize, ty: Type) {
        self.imm(9, (offset + 16) as u64);
        self.emit(0x8b0903a9); // add x9,x29,x9, past saved FP/LR
        self.typed_load_from(ty, 9, 0);
    }
    fn epilogue(&mut self) {
        self.emit(0x910003bf);
        self.emit(0xa8c17bfd);
        self.emit(0xd65f03c0);
    }
    fn narrow(&mut self) {
        self.emit(0x93407c02);
        self.cmp(0, 2);
        self.branch(self.overflow, Branch::Cond(1));
    }
    fn typed_load(&mut self, ty: Type) {
        self.typed_load_from(ty, 0, 0);
    }
    fn typed_load_from(&mut self, ty: Type, base: u32, reg: u32) {
        let opcode = match ty {
            Type::I32 | Type::Rune => 0xb9800000,
            _ if self.types.scalar_enum(ty) => 0xb9800000,
            Type::U32 | Type::F32 => 0xb9400000,
            Type::U16 => 0x79400000,
            Type::Bool | Type::U8 => 0x39400000,
            _ => 0xf9400000,
        };
        self.emit(opcode | (base << 5) | reg);
    }
    fn typed_store(&mut self, ty: Type, base: u32) {
        self.typed_store_reg(ty, base, 0);
    }
    fn typed_store_reg(&mut self, ty: Type, base: u32, reg: u32) {
        self.emit(
            (match ty {
                Type::I32 | Type::U32 | Type::Rune | Type::F32 => 0xb9000000,
                _ if self.types.scalar_enum(ty) => 0xb9000000,
                Type::Bool | Type::U8 => 0x39000000,
                Type::U16 => 0x79000000,
                _ => 0xf9000000,
            }) | base << 5
                | reg,
        );
    }
    fn copy_value(&mut self, storage: usize, ty: Type) {
        self.push(0);
        self.local_addr(storage);
        self.mov(1, 9);
        self.pop(0);
        self.replace_into(ty);
    }
    // Clone before releasing the destination: the source can be the same value
    // or a field within it. The scratch descriptor owns the clone until the
    // final byte copy transfers it into the destination.
    fn replace_into(&mut self, ty: Type) {
        if !self.needs_drop(ty) {
            self.clone_into(ty);
            return;
        }
        let bytes = self.types.storage_bytes(ty).unwrap();
        let scratch = align(bytes, 16);
        self.push(1);
        self.stack_adjust(scratch, true);
        self.sp_addr(0);
        self.mov(1, 9);
        self.clone_into(ty);
        self.push(0);
        self.sp_addr(scratch + 16);
        self.ldr(0, 9);
        self.drop_value(ty);
        self.pop(0);
        self.sp_addr(scratch);
        self.ldr(1, 9);
        self.mov(10, 0);
        self.mov(11, 1);
        self.imm(12, bytes as u64);
        self.copy_bytes(11, 10, 12, 13);
        self.mov(0, 1);
        self.stack_adjust(scratch, false);
        self.pop(1);
    }
    fn release_storage(&mut self, storage: usize, ty: Type) {
        if self.needs_drop(ty) {
            self.local_addr(storage);
            self.mov(0, 9);
            self.drop_value(ty);
        }
    }
    fn retain_string(&mut self, ty: Type) {
        if matches!(ty, Type::String | Type::Bytes | Type::Runes) {
            self.branch(self.strings.retain, Branch::Call);
        }
    }
    // x0 is a borrowed new handle; x1 addresses an initialized owning slot.
    fn replace_string(&mut self) {
        self.branch(self.strings.retain, Branch::Call);
        self.push(0);
        self.ldr(0, 1);
        self.branch(self.strings.release, Branch::Call);
        self.pop(0);
        self.str(0, 1);
    }
    fn store_string(&mut self, slot: usize) {
        self.local_addr(slot);
        self.mov(1, 9);
        self.replace_string();
    }
    fn initialize_owned_storage(&mut self, param_slots: &[usize]) {
        for (slot, ty) in self.owned_storage.clone() {
            if !self.needs_drop(ty) {
                continue;
            }
            // Incoming scalar parameter handles have already been captured.
            if matches!(ty, Type::String | Type::Bytes | Type::Runes) && param_slots.contains(&slot)
            {
                continue;
            }
            self.local_addr(slot);
            self.imm(10, self.types.storage_bytes(ty).unwrap_or(8).max(1) as u64);
            let again = self.label();
            self.mark(again);
            self.strb(31, 9);
            self.add_imm(9, 9, 1);
            self.sub_imm(10, 10, 1);
            self.branch(again, Branch::Nonzero(10));
        }
    }
    fn release_owned_storage(&mut self) {
        for (slot, ty) in self.owned_storage.clone().into_iter().rev() {
            self.release_storage(slot, ty);
        }
    }
    fn index_addr(&mut self, base: &Expr, index: &Expr) {
        let a = self.types.array(base.ty);
        let dynamic = self.types.dynamic_elem(base.ty);
        let fixed = self.locals.fixed_array(base);
        let bounded = fixed
            && matches!(index.kind, ExprKind::Local(slot) if self.locals.bounded_index == Some(slot));
        if fixed {
            self.expr(index);
            if !bounded {
                self.cmp(0, 15);
            }
        } else {
            self.operands(base, index);
            if dynamic.is_some() {
                self.emit(0xf9400422); // ldr x2,[x1,#8]
            } else {
                self.imm(2, a.unwrap().len as u64);
            }
            self.cmp(0, 2);
        }
        if !bounded {
            self.branch(self.bounds, Branch::Cond(2));
        }
        let data = if fixed { 14 } else { 1 };
        let stride = if let Some(elem) = dynamic {
            if !fixed {
                self.ldr(1, 1);
            }
            self.types
                .storage_bytes(elem)
                .unwrap_or_else(|| self.types.scalar_size(elem))
                .max(1)
        } else {
            a.unwrap().elem_size()
        };
        if stride.is_power_of_two() {
            self.emit(0x8b000000 | (data << 5) | (stride.trailing_zeros() << 10));
        // add x0,x1,x0,lsl #log2(stride)
        } else {
            self.imm(2, stride as u64);
            self.emit(0x9b027c00);
            self.add(0, data, 0);
        }
    }
    fn record_value(&mut self, ty: Type, fields: &[(usize, Expr)], storage: usize) {
        self.release_storage(storage, ty);
        self.local_addr(storage);
        self.imm(11, self.types.storage_bytes(ty).unwrap() as u64);
        let zero = self.label();
        self.mark(zero);
        self.emit(0x3900013f);
        self.emit(0x91000529);
        self.emit(0xf100056b);
        self.branch(zero, Branch::Cond(1));
        for (offset, value) in fields {
            self.expr(value);
            self.retain_string(value.ty);
            self.local_addr(storage);
            self.imm(11, *offset as u64);
            self.emit(0x8b0b0129);
            if self.types.storage_bytes(value.ty).is_some() {
                self.mov(1, 9);
                self.clone_into(value.ty);
            } else {
                self.typed_store(value.ty, 9);
            }
        }
        self.local_addr(storage);
        self.mov(0, 9);
    }
    fn expr(&mut self, e: &Expr) {
        if self.optimized_expr(e) {
            return;
        }
        match &e.kind {
            ExprKind::ScratchAlloc {
                area,
                cursor,
                limit,
                count,
                storage,
            } => {
                let elem = self.types.dynamic_elem(e.ty).unwrap();
                self.scratch_alloc(*area, *cursor, *limit, count, *storage, elem);
            }
            ExprKind::ManagedString(value, slot, fresh) => {
                self.expr(value);
                if !fresh {
                    self.retain_string(e.ty);
                }
                self.push(0);
                self.release_storage(*slot, e.ty);
                self.pop(0);
                self.store(*slot, 0);
            }
            ExprKind::IntOp(op, base, arg) => self.integer_expr(*op, base, arg.as_deref()),
            ExprKind::ReferenceAddress(slot) => self.load(*slot),
            ExprKind::LocalAddress(slot) => {
                self.local_addr(*slot);
                self.mov(0, 9);
            }
            ExprKind::Deref(pointer) => {
                self.expr(pointer);
                if self.types.storage_bytes(e.ty).is_none() {
                    self.typed_load(e.ty);
                }
            }
            ExprKind::Offset(base, offset) => {
                self.expr(base);
                self.imm(1, *offset as u64);
                self.add(0, 0, 1);
            }
            ExprKind::ElementAddress(base, index) => self.index_addr(base, index),
            ExprKind::Clone(value, storage) => {
                self.expr(value);
                self.copy_value(*storage, e.ty);
            }
            ExprKind::Dynamic(values, storage) => {
                self.release_storage(*storage, e.ty);
                self.dynamic_value(e.ty, values, *storage);
            }
            ExprKind::DynamicOp(op, base, arg) => {
                self.dynamic_expr(*op, base, arg.as_deref(), e.ty)
            }
            ExprKind::Enum {
                tag,
                fields,
                storage,
            } => {
                if let Some(storage) = storage {
                    self.release_storage(*storage, e.ty);
                }
                self.enum_value(e.ty, *tag, fields, *storage);
            }
            ExprKind::EnumFromInt(value) => {
                self.expr(value);
                let valid = self.label();
                for variant in &self.types.enumeration(e.ty).unwrap().variants {
                    self.imm(1, variant.tag as i64 as u64);
                    self.cmp(0, 1);
                    self.branch(valid, Branch::Cond(0));
                }
                self.branch(self.invalid_enum, Branch::Jump);
                self.mark(valid);
            }
            ExprKind::Float(bits) => self.imm(0, *bits),
            ExprKind::FloatOp(op, value) => self.float_expr(*op, value, e.ty),
            ExprKind::SysCall(op, args, storage) => {
                if let Some(storage) = storage {
                    self.release_storage(*storage, e.ty);
                }
                self.sys_expr(*op, args, *storage);
            }
            ExprKind::StaticBytes { module, site, .. } => {
                self.data_slot(self.static_slots[&(*module, *site)], 0);
            }
            ExprKind::String(s) => {
                let label = self.string_literal(s);
                self.branch(label, Branch::Address(0));
            }
            ExprKind::StringView(base) => self.expr(base),
            ExprKind::StringOp(op, args) => self.string_expr(*op, args),
            ExprKind::Record(fields, storage) => self.record_value(e.ty, fields, *storage),
            ExprKind::Field(base, offset) => {
                self.expr(base);
                self.imm(2, *offset as u64);
                self.emit(0x8b020000);
                if self.types.storage_bytes(e.ty).is_none() {
                    self.typed_load(e.ty);
                }
            }
            ExprKind::Assert(condition) => {
                self.condition(condition, self.assertion, false);
                self.imm(0, 0);
            }
            ExprKind::Range(a, b) => {
                self.expr(a);
                self.push(0);
                self.expr(b);
                self.pop(1);
                self.emit(0xd3607c00);
                self.emit(0x2a0103e1);
                self.emit(0xaa010000);
            }
            ExprKind::Iterator(base) => self.expr(base),
            ExprKind::Len(base, len) => {
                self.expr(base);
                self.imm(0, *len as u64);
            }
            ExprKind::Array(values, storage) => {
                self.release_storage(*storage, e.ty);
                let a = self.types.array(e.ty).unwrap();
                for (i, value) in values.iter().enumerate() {
                    self.expr(value);
                    self.retain_string(value.ty);
                    self.local_addr(*storage);
                    self.imm(11, (i * a.elem_size()) as u64);
                    self.emit(0x8b0b0129);
                    self.typed_store(a.elem, 9);
                }
                self.local_addr(*storage);
                self.mov(0, 9);
            }
            ExprKind::Repeat(value, storage) => {
                self.release_storage(*storage, e.ty);
                let a = self.types.array(e.ty).unwrap();
                self.expr(value);
                self.local_addr(*storage);
                if a.len > 0 {
                    self.imm(11, a.len as u64);
                    let again = self.label();
                    self.mark(again);
                    self.retain_string(a.elem);
                    self.typed_store(a.elem, 9);
                    self.emit(0x91000129 | (a.elem_size() as u32) << 10);
                    self.emit(0xf100056b);
                    self.branch(again, Branch::Cond(1));
                }
                self.local_addr(*storage);
                self.mov(0, 9);
            }
            ExprKind::Index(base, index) => {
                let key = self.locals.read_key(base, index);
                if key.is_some() && key == self.locals.last_read {
                    self.mov(0, 16);
                } else {
                    self.index_addr(base, index);
                    if self.types.storage_bytes(e.ty).is_none() {
                        self.typed_load(e.ty);
                    }
                    if key.is_some() {
                        self.mov(16, 0);
                        self.locals.last_read = key;
                    }
                }
            }
            ExprKind::Number(n) => self.imm(0, *n as u64),
            ExprKind::Bool(b) => self.imm(0, u64::from(*b)),
            ExprKind::Local(_) => self.simple_into(e, 0),
            ExprKind::Function(f) if self.foreign_slots[*f].is_some() => {
                self.data_slot(self.foreign_slots[*f].unwrap(), 0);
                self.ldr(0, 0);
            }
            ExprKind::Callback(f) => self.branch(self.callbacks[*f].unwrap(), Branch::Address(0)),
            ExprKind::Call(target, args, storage) if self.types.is_extern_fn(target.ty) => {
                self.foreign_call(target, args, e.ty, *storage);
            }
            ExprKind::Function(f) => self.branch(self.functions[*f], Branch::Address(0)),
            ExprKind::Call(target, args, storage) => {
                if let ExprKind::Function(f) = target.kind {
                    if let Some(body) = self.leaves[f] {
                        self.inline_call(body, args, *storage);
                        return;
                    }
                }
                if let Some(storage) = storage {
                    self.release_storage(*storage, e.ty);
                }
                let direct = if let ExprKind::Function(f) = &target.kind {
                    Some(*f)
                } else {
                    None
                };
                if direct.is_none() {
                    self.expr(target);
                    self.push(0);
                }
                let (locations, outgoing) =
                    arguments(&args.iter().map(|a| a.ty).collect::<Vec<_>>(), self.types);
                let argument_bytes = if outgoing == 0 && args.iter().all(Self::simple) {
                    for (arg, location) in args.iter().zip(locations) {
                        match location {
                            Argument::General(reg) => self.simple_into(arg, reg),
                            Argument::Float(reg) => {
                                self.simple_into(arg, 10);
                                self.bits_to_fp(reg, 10, arg.ty);
                            }
                            Argument::Stack(_) => unreachable!(),
                        }
                    }
                    0
                } else {
                    for arg in args {
                        self.expr(arg);
                        self.push(0);
                    }
                    self.stack_adjust(outgoing, true);
                    for (arg, location) in locations.into_iter().enumerate() {
                        self.sp_addr(outgoing + (args.len() - 1 - arg) * 16);
                        match location {
                            Argument::General(reg) => self.emit(0xf9400120 | reg),
                            Argument::Float(reg) => {
                                self.emit(0xf940012a);
                                self.bits_to_fp(reg, 10, args[arg].ty);
                            }
                            Argument::Stack(offset) => {
                                self.emit(0xf940012a);
                                self.sp_addr(offset);
                                self.typed_store_reg(args[arg].ty, 9, 10);
                            }
                        }
                    }
                    outgoing + args.len() * 16
                };
                if let Some(storage) = storage {
                    self.local_addr(*storage);
                    self.mov(8, 9);
                }
                if let Some(f) = direct {
                    self.branch(self.functions[f], Branch::Call);
                } else {
                    self.sp_addr(argument_bytes);
                    self.emit(0xf9400131);
                    self.emit(0xd63f0220); // blr x17; x8 is the aggregate result address
                }
                if e.ty.float() {
                    self.fp_to_bits(0, 0, e.ty);
                }
                if let Some(storage) = storage {
                    self.local_addr(*storage);
                    self.mov(0, 9);
                }
                self.stack_adjust(
                    argument_bytes + if direct.is_none() { 16 } else { 0 },
                    false,
                );
            }
            ExprKind::Print(x) => {
                self.expr(x);
                if x.ty.float() {
                    self.float_to_string(x.ty);
                    self.push(0);
                }
                if x.ty.integer() {
                    self.imm(8, u64::from(x.ty.unsigned()));
                }
                self.branch(
                    match x.ty {
                        Type::Bool => self.print_bool,
                        Type::String => self.strings.print,
                        Type::Rune => self.strings.print_rune,
                        Type::F32 | Type::F64 => self.strings.print,
                        _ => self.print_int,
                    },
                    Branch::Call,
                );
                if x.ty.float() {
                    self.pop(0);
                    self.branch(self.strings.release, Branch::Call);
                }
                self.imm(0, 0);
            }
            ExprKind::Cast(x) => {
                self.expr(x);
                if e.ty.float() || x.ty.float() {
                    self.float_cast(x.ty, e.ty);
                } else {
                    self.checked_integer_cast(x.ty, e.ty);
                }
            }
            ExprKind::Unary(op, x) => {
                self.expr(x);
                if e.ty.float() {
                    self.bits_to_fp(0, 0, e.ty);
                    self.emit(0x1e214000 | Self::fp_width(e.ty));
                    self.fp_to_bits(0, 0, e.ty);
                    return;
                }
                if *op == "!" {
                    self.imm(1, 1);
                    self.emit(0xca010000);
                } else if e.ty.unsigned() {
                    self.branch(self.overflow, Branch::Nonzero(0));
                } else {
                    self.emit(if e.ty == Type::I32 {
                        0x6b0003e0
                    } else {
                        0xeb0003e0
                    });
                    self.branch(self.overflow, Branch::Cond(6));
                    if e.ty == Type::I32 {
                        self.emit(0x93407c00);
                    }
                }
            }
            ExprKind::Binary(op, a, b) if matches!(*op, "&&" | "||") => {
                let end = self.label();
                self.expr(a);
                self.branch(
                    end,
                    if *op == "&&" {
                        Branch::Zero(0)
                    } else {
                        Branch::Nonzero(0)
                    },
                );
                self.expr(b);
                self.mark(end);
            }
            ExprKind::Binary(op, a, b) => {
                self.operands(a, b);
                if a.ty.float() {
                    self.float_binary(op, a.ty);
                    return;
                }
                if a.ty.unsigned() {
                    self.unsigned_binary(op, a.ty);
                    return;
                }
                if a.ty == Type::String {
                    self.branch(
                        if *op == "+" {
                            self.strings.concat
                        } else {
                            self.strings.compare
                        },
                        Branch::Call,
                    );
                    if *op != "+" {
                        self.cmp(0, 31);
                        let condition = match *op {
                            "==" => 0,
                            "!=" => 1,
                            "<" => 11,
                            "<=" => 13,
                            ">" => 12,
                            ">=" => 10,
                            _ => unreachable!(),
                        };
                        self.emit(0x9a9f07e0 | (condition ^ 1) << 12);
                    }
                    return;
                }
                match *op {
                    "+" | "-" => {
                        let word = match (*op, e.ty) {
                            ("+", Type::I32) => 0x2b000020,
                            ("-", Type::I32) => 0x6b000020,
                            ("+", _) => 0xab000020,
                            _ => 0xeb000020,
                        };
                        self.emit(word);
                        self.branch(self.overflow, Branch::Cond(6));
                        if e.ty == Type::I32 {
                            self.emit(0x93407c00);
                        }
                    }
                    "*" => {
                        if e.ty == Type::I64 {
                            self.emit(0x9b407c22);
                        } // smulh x2,x1,x0
                        self.emit(0x9b007c20);
                        if e.ty == Type::I32 {
                            self.narrow();
                        } else {
                            self.emit(0x937ffc03);
                            self.cmp(2, 3);
                            self.branch(self.overflow, Branch::Cond(1));
                        }
                    }
                    "/" | "%" => {
                        self.branch(self.divide_zero, Branch::Zero(0));
                        let valid = self.label();
                        self.imm(
                            2,
                            if e.ty == Type::I32 {
                                i32::MIN as i64 as u64
                            } else {
                                i64::MIN as u64
                            },
                        );
                        self.cmp(1, 2);
                        self.branch(valid, Branch::Cond(1));
                        self.imm(2, u64::MAX);
                        self.cmp(0, 2);
                        self.branch(self.overflow, Branch::Cond(0));
                        self.mark(valid);
                        self.emit(0x9ac00c22); // sdiv x2,x1,x0
                        if *op == "/" {
                            self.mov(0, 2);
                        } else {
                            self.emit(0x9b008440);
                        } // msub x0,x2,x0,x1
                    }
                    _ => {
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
                        self.emit(0x9a9f07e0 | (condition ^ 1) << 12);
                    }
                }
            }
        }
    }
    fn stmts(&mut self, body: &[Stmt]) {
        for (position, stmt) in body.iter().enumerate() {
            match stmt {
                Stmt::StorePlace(address, value) => {
                    if let ExprKind::Index(base, index) = &value.kind {
                        let key = self.locals.read_key(base, index);
                        if key.is_some()
                            && key == self.locals.last_read
                            && matches!(&address.kind, ExprKind::ElementAddress(base, idx) if self.locals.fixed_array(base) && optimize::Subscript::new(idx).is_some())
                        {
                            self.expr(address);
                            self.mov(1, 0);
                            self.mov(0, 16);
                        } else {
                            self.operands(address, value);
                        }
                    } else {
                        self.operands(address, value);
                    }
                    self.locals.last_read = None; // may overwrite a cached element
                    if self.types.storage_bytes(value.ty).is_some() {
                        self.replace_into(value.ty);
                    } else if matches!(value.ty, Type::String | Type::Bytes | Type::Runes) {
                        self.replace_string();
                    } else {
                        self.typed_store(value.ty, 1);
                    }
                }
                Stmt::Match {
                    source,
                    slot,
                    storage,
                    arms,
                } => self.enum_match(source, *slot, *storage, arms),
                Stmt::Store(slot, e) => {
                    if let Some(home) = self.locals.register(*slot) {
                        self.expr_into(e, home);
                        self.locals.write_slot(*slot);
                        continue;
                    }
                    if self.take_string(e, *slot) {
                        self.push(0);
                        self.release_storage(*slot, e.ty);
                        self.pop(0);
                        self.store(*slot, 0);
                        continue;
                    }
                    self.expr(e);
                    if matches!(e.ty, Type::String | Type::Bytes | Type::Runes) {
                        self.store_string(*slot);
                    } else {
                        self.store(*slot, 0);
                    }
                }
                Stmt::StoreAggregate(slot, storage, e) => {
                    // A small constructor consumes only scalar parameters. Its
                    // arguments are captured before it writes this destination,
                    // including assignments such as p = point(p.y, p.x).
                    if self.record_into(e, *storage) {
                        self.store(*slot, 0);
                        continue;
                    }
                    self.expr(e);
                    if self.can_move_buffer(e, *slot) {
                        self.push(0);
                        self.release_storage(*storage, e.ty);
                        self.local_addr(*storage);
                        self.mov(1, 9);
                        self.pop(0);
                        self.move_buffer();
                    } else {
                        self.copy_value(*storage, e.ty);
                    }
                    self.store(*slot, 0);
                }
                Stmt::Return(e) => {
                    if let Some(e) = e {
                        self.return_expr(e);
                        if self.types.storage_bytes(e.ty).is_some() {
                            self.push(0);
                            self.load(self.return_slot.expect("aggregate return destination"));
                            self.mov(1, 0);
                            self.pop(0);
                            if matches!(e.ty, Type::Dynamic(_))
                                && matches!(
                                    e.kind,
                                    ExprKind::Local(_)
                                        | ExprKind::Dynamic(_, _)
                                        | ExprKind::Call(_, _, Some(_))
                                )
                            {
                                self.move_buffer();
                            } else {
                                self.clone_into(e.ty);
                            }
                        }
                    } else {
                        self.imm(0, 0);
                    }
                    if self
                        .owned_storage
                        .iter()
                        .any(|(_, ty)| self.needs_drop(*ty))
                    {
                        self.push(0);
                        self.release_owned_storage();
                        self.pop(0);
                    }
                    if let Some(e) = e {
                        if e.ty.float() {
                            self.bits_to_fp(0, 0, e.ty);
                        }
                    }
                    self.function_epilogue();
                }
                Stmt::Expr(e) => self.expr(e),
                Stmt::Block(b) => self.stmts(b),
                Stmt::DropStorage(owners) => {
                    for &(slot, ty) in owners.iter().rev() {
                        self.release_storage(slot, ty);
                    }
                }
                Stmt::If(c, yes, no) => {
                    if let Some(value) = optimize::constant(c) {
                        self.stmts(if value != 0 { yes } else { no });
                        continue;
                    }
                    let other = self.label();
                    let end = self.label();
                    self.condition(c, other, false);
                    self.stmts(yes);
                    self.branch(end, Branch::Jump);
                    self.mark(other);
                    self.stmts(no);
                    self.mark(end);
                }
                Stmt::While(c, b) => {
                    let previous = position.checked_sub(1).map(|n| &body[n]);
                    if let Some(index) = self.locals.scan_index(previous, c, b) {
                        let test = self.label();
                        let work = self.label();
                        let done = self.label();
                        self.loops.push((test, done));
                        self.branch(test, Branch::Jump);
                        self.mark(work);
                        self.locals.bounded_index = Some(index);
                        self.stmts(b);
                        self.locals.bounded_index = None;
                        self.mark(test);
                        self.condition(c, work, true);
                        self.mark(done);
                        self.loops.pop();
                        continue;
                    }
                    let head = self.label();
                    let end = self.label();
                    self.loops.push((head, end));
                    self.mark(head);
                    self.condition(c, end, false);
                    self.stmts(b);
                    self.branch(head, Branch::Jump);
                    self.mark(end);
                    self.loops.pop();
                }
                Stmt::For {
                    source,
                    source_slot,
                    cursor,
                    end,
                    binding,
                    storage,
                    body,
                } => {
                    self.expr(source);
                    if let Some(storage) = storage {
                        self.copy_value(*storage, source.ty);
                    }
                    self.store(*source_slot, 0);
                    if source.ty == Type::Range {
                        self.emit(0x93407c00);
                        self.store(*cursor, 0);
                        self.load(*source_slot);
                        self.emit(0x9360fc00);
                        self.store(*end, 0);
                    } else {
                        self.imm(0, 0);
                        self.store(*cursor, 0);
                        if matches!(source.ty, Type::Bytes | Type::Runes) {
                            self.load(*source_slot);
                            self.emit(0xf9400000);
                        } else if self.types.dynamic_elem(source.ty).is_some() {
                            self.load(*source_slot);
                            self.add_imm(0, 0, 8);
                            self.ldr(0, 0);
                        } else {
                            self.imm(0, self.types.array(source.ty).unwrap().len as u64);
                        }
                        self.store(*end, 0);
                    }
                    let head = self.label();
                    let next = self.label();
                    let done = self.label();
                    self.loops.push((next, done));
                    self.mark(head);
                    self.load_reg(*cursor, 1);
                    self.load(*end);
                    self.cmp(1, 0);
                    self.branch(done, Branch::Cond(10));
                    if matches!(source.ty, Type::Bytes | Type::Runes) {
                        self.load_reg(*source_slot, 2);
                        self.load_reg(*cursor, 0);
                        self.add(0, 2, 0);
                        self.add_imm(0, 0, 8); // byte data follows the length
                        if source.ty == Type::Runes {
                            self.branch(self.strings.decode, Branch::Call);
                        } else {
                            self.ldrb(0, 0);
                            self.imm(1, 1);
                        }
                        // Advance before the body. x0 retains the decoded scalar.
                        self.load_reg(*cursor, 2);
                        self.add(2, 2, 1);
                        self.store(*cursor, 2);
                    } else if self.types.array(source.ty).is_some()
                        || self.types.dynamic_elem(source.ty).is_some()
                    {
                        self.load_reg(*source_slot, 1);
                        self.load_reg(*cursor, 0);
                        let (elem, stride) = if let Some(a) = self.types.array(source.ty) {
                            (a.elem, a.elem_size())
                        } else {
                            self.ldr(1, 1);
                            let elem = self.types.dynamic_elem(source.ty).unwrap();
                            (elem, self.types.scalar_size(elem))
                        };
                        if stride.is_power_of_two() {
                            self.emit(0x8b000020 | stride.trailing_zeros() << 10);
                        } else {
                            self.imm(2, stride as u64);
                            self.emit(0x9b027c00);
                            self.add(0, 1, 0);
                        }
                        self.typed_load(elem);
                    } else {
                        self.load(*cursor);
                    }
                    let string_item = self
                        .types
                        .array(source.ty)
                        .map(|a| a.elem)
                        .or_else(|| self.types.dynamic_elem(source.ty))
                        .filter(|ty| matches!(ty, Type::String | Type::Bytes | Type::Runes));
                    if string_item.is_some() {
                        self.store_string(*binding);
                    } else {
                        self.store(*binding, 0);
                    }
                    self.stmts(body);
                    self.mark(next);
                    if !matches!(source.ty, Type::Bytes | Type::Runes) {
                        self.load(*cursor);
                        self.emit(0x91000400);
                        self.store(*cursor, 0);
                    }
                    self.branch(head, Branch::Jump);
                    self.mark(done);
                    if let Some(ty) = string_item {
                        self.release_storage(*binding, ty);
                    }
                    if let Some(storage) = storage {
                        self.release_storage(*storage, source.ty);
                    }
                    self.loops.pop();
                }
                Stmt::Break => self.branch(self.loops.last().unwrap().1, Branch::Jump),
                Stmt::Continue => self.branch(self.loops.last().unwrap().0, Branch::Jump),
            }
        }
    }
    fn runtime(&mut self) {
        self.string_runtime();
        self.sys_runtime();
        self.mark(self.print_int);
        self.prologue(64);
        self.emit(0x9100ffe1); // add x1,sp,#63
        self.imm(9, 10);
        self.emit(0x39000029);
        self.imm(2, 1);
        self.mov(3, 0);
        self.imm(4, 10);
        self.imm(7, 0);
        self.cmp(3, 31);
        let magnitude = self.label();
        self.branch(magnitude, Branch::Nonzero(8));
        self.branch(magnitude, Branch::Cond(10));
        self.imm(7, 1);
        self.emit(0xcb0303e3); // unsigned magnitude, including i64::MIN
        self.mark(magnitude);
        let digit = self.label();
        self.mark(digit);
        self.emit(0x9ac40865); // udiv x5,x3,x4
        self.emit(0x9b048ca6); // msub x6,x5,x4,x3
        self.emit(0x1100c0c6); // add w6,w6,#48
        self.emit(0xd1000421);
        self.emit(0x39000026);
        self.emit(0x91000442);
        self.mov(3, 5);
        self.branch(digit, Branch::Nonzero(3));
        let signed = self.label();
        self.branch(signed, Branch::Zero(7));
        self.emit(0xd1000421);
        self.imm(9, 45);
        self.emit(0x39000029);
        self.emit(0x91000442);
        self.mark(signed);
        self.branch(self.write, Branch::Call);
        self.epilogue();

        self.mark(self.print_bool);
        self.prologue(16);
        let falsity = self.label();
        let output = self.label();
        self.branch(falsity, Branch::Zero(0));
        self.imm(9, u64::from_le_bytes(*b"true\n\0\0\0"));
        self.imm(2, 5);
        self.branch(output, Branch::Jump);
        self.mark(falsity);
        self.imm(9, u64::from_le_bytes(*b"false\n\0\0"));
        self.imm(2, 6);
        self.mark(output);
        self.emit(0xf90003e9);
        self.emit(0x910003e1);
        self.branch(self.write, Branch::Call);
        self.epilogue();

        self.mark(self.write);
        let again = self.label();
        self.mark(again);
        self.imm(0, 1);
        self.syscall(4);
        self.branch(self.io_error, Branch::Cond(2));
        self.branch(self.io_error, Branch::Zero(0));
        self.emit(0xcb000042);
        self.emit(0x8b000021);
        self.branch(again, Branch::Nonzero(2));
        self.emit(0xd65f03c0);
        for (label, status) in [
            (self.overflow, 101),
            (self.divide_zero, 102),
            (self.io_error, 74),
            (self.bounds, 103),
            (self.assertion, 104),
            (self.float_conversion, 107),
            (self.invalid_enum, 108),
        ] {
            self.mark(label);
            self.imm(0, status);
            self.syscall(1);
            self.emit(0xd4200000);
        }
    }
    fn finish(mut self) -> Result<Vec<u8>, String> {
        if self.target == Target::X86_64Linux {
            if !self.data_fixups.is_empty() {
                return crate::x86_64::lower_dynamic(
                    &self.words,
                    &self.labels,
                    &self.fixups,
                    self.data_start,
                    &self.data_fixups,
                );
            }
            return crate::x86_64::lower(&self.words, &self.labels, &self.fixups, self.data_start);
        }
        for (at, label, kind) in self.fixups {
            let to = self.labels[label].ok_or("unresolved function or branch")?;
            let delta = to as i64 - at as i64;
            if let Branch::Address(reg) = kind {
                // Address fixups use the selected output image layout.
                let from = self.text_offset + at * 4;
                let target = self.text_offset + to * 4;
                let pages = (target / 4096) as i64 - (from / 4096) as i64;
                if !(-(1 << 20)..(1 << 20)).contains(&pages) {
                    return Err("address exceeds ADRP range".into());
                }
                let bits = pages as u32 & 0x1fffff;
                self.words[at] = 0x90000000 | (bits & 3) << 29 | (bits >> 2) << 5 | reg;
                self.words[at + 1] = 0x91000000 | ((target as u32 & 4095) << 10) | reg << 5 | reg;
                continue;
            }
            let (base, bits) = match kind {
                Branch::Jump => (0x14000000, 26),
                Branch::Call => (0x94000000, 26),
                Branch::Cond(c) => (0x54000000 | c, 19),
                Branch::Zero(r) => (0xb4000000 | r, 19),
                Branch::Nonzero(r) => (0xb5000000 | r, 19),
                Branch::Address(_) => unreachable!(),
            };
            if delta < -(1i64 << (bits - 1)) || delta >= 1i64 << (bits - 1) {
                if bits == 26 || !(-(1 << 25)..(1 << 25)).contains(&(delta - 1)) {
                    return Err("program exceeds branch range".into());
                }
                self.words[at] = (base
                    ^ if matches!(kind, Branch::Cond(_)) {
                        1
                    } else {
                        0x01000000
                    })
                    | (2 << 5);
                self.words[at + 1] = 0x14000000 | ((delta - 1) as u32 & 0x3ffffff);
                continue;
            }
            let imm = (delta as u32) & ((1 << bits) - 1);
            self.words[at] = base | if bits == 26 { imm } else { imm << 5 };
        }
        let data = if self.target == Target::Arm64Macos {
            image::dynamic_data_offset(self.text_offset, self.words.len() * 4)
        } else {
            elf::dynamic_data_offset(self.words.len() * 4)
        };
        for (at, slot, reg) in self.data_fixups {
            let from = self.text_offset + at * 4;
            let target = data + slot * 8;
            let pages = (target / 4096) as i64 - (from / 4096) as i64;
            if !(-(1 << 20)..(1 << 20)).contains(&pages) {
                return Err("runtime global address exceeds ADRP range".into());
            }
            let bits = pages as u32 & 0x1fffff;
            self.words[at] = 0x90000000 | (bits & 3) << 29 | (bits >> 2) << 5 | reg;
            self.words[at + 1] = 0x91000000 | ((target as u32 & 4095) << 10) | reg << 5 | reg;
        }
        Ok(self.words.iter().flat_map(|w| w.to_le_bytes()).collect())
    }
}

// Assign writable image storage before emitting functions so warm cache hits and
// inlined calls resolve every syntactic allocation to the same address.
fn plan_static_storage(
    code: &mut Code<'_>,
    program: &Program,
    reachable: &[bool],
) -> Result<usize, String> {
    let imports = code.import_count;
    // Reserve image cells used by native runtime bookkeeping and callback entry.
    let mut offset = (imports + if code.has_callbacks { 9 } else { 2 }) * 8;
    for (index, function) in program.functions.iter().enumerate() {
        if !reachable[index] {
            continue;
        }
        let mut mismatch = None;
        reachable::visit_body(&function.body, false, &mut |expr, _| {
            if matches!(expr.kind, ExprKind::SysCall(..)) {
                code.sys_types = program.sys_types;
            }
            if let ExprKind::StaticBytes {
                module,
                site,
                bytes,
                platform,
            } = &expr.kind
            {
                if (platform == "macos" && code.target != Target::Arm64Macos)
                    || (platform == "linux" && code.target == Target::Arm64Macos)
                {
                    mismatch = Some(if platform == "linux" {
                        "Linux"
                    } else {
                        "macOS"
                    });
                }
                code.static_slots
                    .entry((*module, *site))
                    .or_insert_with(|| {
                        offset = align(offset, 16);
                        let slot = offset / 8;
                        offset += *bytes;
                        slot
                    });
            }
        });
        if let Some(required) = mismatch {
            return Err(format!(
                "native module requires {required}; selected target is {}",
                code.target
            ));
        }
    }
    Ok(offset - imports * 8)
}

pub fn compile(program: &Program) -> Result<Vec<u8>, String> {
    compile_for(program, Target::host()?)
}
pub fn compile_for(program: &Program, target: Target) -> Result<Vec<u8>, String> {
    compile_entry_for(
        program,
        program.main.ok_or("program has no main entrypoint")?,
        target,
    )
}
pub fn compile_entry(program: &Program, entry: usize) -> Result<Vec<u8>, String> {
    compile_entry_for(program, entry, Target::host()?)
}
pub fn compile_entry_for(
    program: &Program,
    entry: usize,
    target: Target,
) -> Result<Vec<u8>, String> {
    compile_cached(program, entry, target, None)
}

fn compile_cached(
    program: &Program,
    entry: usize,
    target: Target,
    mut cache: Option<&mut Incremental>,
) -> Result<Vec<u8>, String> {
    if program
        .functions
        .get(entry)
        .is_none_or(|f| !f.params.is_empty())
    {
        return Err("entrypoint must be a function with no parameters".into());
    }
    if target == Target::Wasm32 {
        return crate::wasm::compile(program, entry, &reachable::functions(program, entry));
    }
    let mut code = Code::new(program.functions.len(), &program.types);
    code.target = target;
    code.leaves = program
        .functions
        .iter()
        .map(optimize::inline_body)
        .collect();
    let reachable = reachable::functions(program, entry);
    code.float_formatter = program.float_formatter.map(|f| code.functions[f]);
    let imports = ffi::Imports::collect(&mut code, program, &reachable)?;
    code.prepare_callbacks(program, &reachable);
    let global_bytes = plan_static_storage(&mut code, program, &reachable)?;
    let dynamic =
        !imports.symbols.is_empty() || code.has_callbacks || !code.static_slots.is_empty();
    code.text_offset = match target {
        Target::Arm64Macos if dynamic => image::text_offset(&imports),
        Target::Arm64Macos => 432,
        _ => elf::TEXT_OFFSET,
    };
    if target == Target::Arm64Macos {
        code.mov(23, 0); // LC_MAIN argc and argv, retained across Dunder calls
        code.mov(24, 1);
    } else {
        code.emit(0xf94003f7); // ELF entry: ldr x23,[sp] (argc)
        code.emit(0x910023f8); // add x24,sp,#8 (argv)
    }
    code.emit(0xd51b441f); // msr fpcr,xzr: IEEE rounding and gradual underflow
                           // Runtime registers are preserved by the system ABI at native calls.
    code.imm(19, 0); // lazily mapped string arena cursor
    code.imm(20, 0); // string arena end
    code.imm(21, 0); // lazily mapped dynamic storage arena
    code.imm(22, 0);
    code.branch(code.functions[entry], Branch::Call);
    code.syscall(1);
    code.emit(0xd4200000);
    let global_labels = code.labels.len();
    let import_key = cache.as_ref().map(|_| {
        fingerprint::digest((
            &code.foreign_slots,
            code.import_count,
            &code.callbacks,
            &code.static_slots,
        ))
    });
    for (i, f) in program.functions.iter().enumerate() {
        if !reachable[i] || f.external.is_some() {
            continue;
        }
        code.return_slot = f.return_slot;
        code.owned_storage = f.owned_storage.clone();
        code.mark(code.functions[i]);
        let recording = if let Some(cache) = &mut cache {
            let path = cache.path(i, program, import_key.as_ref().unwrap());
            if cache::restore(&mut code, &path, global_labels).is_some() {
                cache.stats.hits += 1;
                continue;
            }
            cache.stats.misses += 1;
            Some((path, cache::Mark::new(&mut code)))
        } else {
            None
        };
        code.locals = optimize::LocalPlan::new(f, &code.leaves, target, &program.types);
        code.prologue((f.slots + code.locals.inline_slots) * 8);
        for reg in code.locals.registers() {
            code.push(reg);
        }
        if let Some(slot) = f.return_slot {
            code.store(slot, 8);
        }
        let (locations, _) = arguments(&f.params, &program.types);
        for (param, location) in locations.iter().enumerate() {
            if let Argument::General(reg) = location {
                code.store(f.param_slots[param], *reg);
            }
        }
        for (param, location) in locations.iter().enumerate() {
            match location {
                Argument::Float(reg) => {
                    code.fp_to_bits(0, *reg, f.params[param]);
                    code.store(f.param_slots[param], 0);
                }
                Argument::Stack(offset) => {
                    code.load_stack_param(*offset, f.params[param]);
                    code.store(f.param_slots[param], 0);
                }
                Argument::General(_) => {}
            }
        }
        code.initialize_owned_storage(&f.param_slots);
        for (param, ty) in f.params.iter().enumerate() {
            if matches!(ty, Type::String | Type::Bytes | Type::Runes) {
                code.load(f.param_slots[param]);
                code.retain_string(*ty);
            }
        }
        for (param, storage) in f.param_storage.iter().enumerate() {
            if let Some(storage) = storage {
                code.load(f.param_slots[param]);
                code.copy_value(*storage, f.params[param]);
                code.store(f.param_slots[param], 0);
            }
        }
        if let Some(slot) = code.locals.array_parameter {
            code.load(slot);
            code.ldr(14, 0); // immutable descriptor data pointer
            code.emit(0xf940040f); // ldr x15,[x0,#8] -- its live element count
        }
        code.stmts(&f.body);
        code.release_owned_storage();
        code.imm(0, 0);
        code.function_epilogue();
        code.locals = optimize::LocalPlan::default();
        if let Some((path, start)) = recording {
            cache::save(&mut code, start, &path, global_labels)
                .map_err(|e| format!("saving function cache: {e}"))?;
        }
    }
    code.callback_entries(program);
    code.runtime();
    code.collections_runtime();
    code.data_start = code.words.len();
    code.string_literals();
    let text = code.finish()?;
    Ok(match target {
        Target::Arm64Macos if dynamic => {
            image::macho_imports_with_storage(&text, &imports, global_bytes)
        }
        Target::Arm64Macos => macho(&text),
        Target::Arm64Linux if dynamic => {
            elf::dynamic_executable(&text, 183, &imports, global_bytes.max(4096))
        }
        Target::X86_64Linux if dynamic => {
            elf::dynamic_executable(&text, 62, &imports, global_bytes.max(4096))
        }
        Target::Wasm32 => unreachable!("wasm32 uses its structured backend"),
        Target::Arm64Linux => elf::executable(&text, 183),
        Target::X86_64Linux => elf::executable(&text, 62),
    })
}

// Darwin packs stack arguments at their natural size and alignment, then
// requires the caller's SP to remain 16-byte aligned, including native calls.
enum Argument {
    General(u32),
    Float(u32),
    Stack(usize),
}
fn arguments(types: &[Type], layouts: &Types) -> (Vec<Argument>, usize) {
    let mut next = 0;
    let mut locations = Vec::new();
    let mut general = 0;
    let mut fp = 0;
    for ty in types {
        if ty.float() && fp < 8 {
            locations.push(Argument::Float(fp));
            fp += 1;
            continue;
        }
        if !ty.float() && general < 8 {
            locations.push(Argument::General(general));
            general += 1;
            continue;
        }
        let size = layouts.scalar_size(*ty);
        next = align(next, size);
        locations.push(Argument::Stack(next));
        next += size;
    }
    (locations, align(next, 16))
}

fn align(n: usize, to: usize) -> usize {
    (n + to - 1) & !(to - 1)
}
fn u32le(v: &mut Vec<u8>, n: u32) {
    v.extend(n.to_le_bytes());
}
fn u64le(v: &mut Vec<u8>, n: u64) {
    v.extend(n.to_le_bytes());
}
fn name(v: &mut Vec<u8>, s: &str) {
    let mut b = [0u8; 16];
    b[..s.len()].copy_from_slice(s.as_bytes());
    v.extend(b);
}
fn segment(
    v: &mut Vec<u8>,
    n: &str,
    mapping: [u64; 4], // vmaddr, vmsize, fileoff, filesize
    prot: u32,
    sections: u32,
) {
    u32le(v, 0x19);
    u32le(v, 72 + 80 * sections);
    name(v, n);
    for n in mapping {
        u64le(v, n);
    }
    for n in [prot, prot, sections, 0] {
        u32le(v, n);
    }
}
fn be32(v: &mut [u8], at: usize, n: u32) {
    v[at..at + 4].copy_from_slice(&n.to_be_bytes());
}
fn be64(v: &mut [u8], at: usize, n: u64) {
    v[at..at + 8].copy_from_slice(&n.to_be_bytes());
}

fn macho(text: &[u8]) -> Vec<u8> {
    const PAGE: usize = 16384;
    const VM: u64 = 0x100000000;
    const COMMANDS: usize = 392;
    let text_off = align(32 + COMMANDS, 16);
    let text_end = text_off + text.len();
    let sig_off = align(text_end, PAGE);
    let slots = sig_off / PAGE;
    let ident = b"dunder\0";
    let hash_off = 0x58 + ident.len();
    let cd_len = hash_off + slots * 32;
    let sig_size = align(20 + cd_len, 16);
    let mut image = Vec::new();
    for n in [
        0xfeedfacf,
        0x0100000c,
        0,
        2,
        7,
        COMMANDS as u32,
        0x00200005,
        0,
    ] {
        u32le(&mut image, n);
    }
    segment(&mut image, "__PAGEZERO", [0, VM, 0, 0], 0, 0);
    segment(
        &mut image,
        "__TEXT",
        [VM, sig_off as u64, 0, text_end as u64],
        5,
        1,
    );
    name(&mut image, "__text");
    name(&mut image, "__TEXT");
    u64le(&mut image, VM + text_off as u64);
    u64le(&mut image, text.len() as u64);
    for n in [text_off as u32, 4, 0, 0, 0x80000400, 0, 0, 0] {
        u32le(&mut image, n);
    }
    segment(
        &mut image,
        "__LINKEDIT",
        [
            VM + sig_off as u64,
            align(sig_size, PAGE) as u64,
            sig_off as u64,
            sig_size as u64,
        ],
        1,
        0,
    );
    for n in [0x32, 24, 1, 0xb0000, 0xb0000, 0] {
        u32le(&mut image, n);
    }
    for n in [0xe, 32, 12] {
        u32le(&mut image, n);
    }
    let mut dyld = [0u8; 20];
    dyld[..13].copy_from_slice(b"/usr/lib/dyld");
    image.extend(dyld);
    u32le(&mut image, 0x80000028);
    u32le(&mut image, 24);
    u64le(&mut image, text_off as u64);
    u64le(&mut image, 0);
    for n in [0x1d, 16, sig_off as u32, sig_size as u32] {
        u32le(&mut image, n);
    }
    assert_eq!(image.len(), 32 + COMMANDS);
    image.resize(text_off, 0);
    image.extend(text);
    image.resize(sig_off, 0);
    let mut cd = vec![0u8; cd_len];
    for (at, n) in [
        (0, 0xfade0c02),
        (4, cd_len as u32),
        (8, 0x20400),
        (12, 2),
        (16, hash_off as u32),
        (20, 0x58),
        (28, slots as u32),
        (32, sig_off as u32),
    ] {
        be32(&mut cd, at, n);
    }
    cd[36] = 32;
    cd[37] = 2;
    cd[39] = 14;
    be64(&mut cd, 64, 0);
    be64(&mut cd, 72, sig_off as u64);
    be64(&mut cd, 80, 1);
    cd[0x58..hash_off].copy_from_slice(ident);
    for i in 0..slots {
        cd[hash_off + i * 32..hash_off + (i + 1) * 32]
            .copy_from_slice(&sha256(&image[i * PAGE..(i + 1) * PAGE]));
    }
    let mut sig = vec![0u8; 20];
    for (at, n) in [
        (0, 0xfade0cc0),
        (4, sig_size as u32),
        (8, 1),
        (12, 0),
        (16, 20),
    ] {
        be32(&mut sig, at, n);
    }
    sig.extend(cd);
    sig.resize(sig_size, 0);
    image.extend(sig);
    image
}

/// Seal the exact bundle metadata bytes using the compiler's native signer.
pub fn sign_bundle(
    image: &mut Vec<u8>,
    identifier: &str,
    info: &[u8],
    resources: &[u8],
) -> Result<(), String> {
    image::sign_bundle(image, identifier, info, resources)
}

/// SHA-256 for bundle resource seals; no external signing tool is needed.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend((data.len() as u64 * 8).to_be_bytes());
    for block in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let a = w[i - 15];
            let b = w[i - 2];
            let s0 = a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3);
            let s1 = b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (i, s) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&s.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn large_images_map_and_verify_a_signature_spanning_multiple_pages() {
        use std::{fs, os::unix::fs::PermissionsExt, process::Command};
        struct Binary(std::path::PathBuf);
        impl Drop for Binary {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let binary =
            Binary(std::env::temp_dir().join(format!("dunder-large-image-{}", std::process::id())));
        let types = Types::default();
        let mut code = Code::new(0, &types);
        code.imm(0, 42);
        code.imm(16, 0x2000001);
        code.emit(0xd4001001);
        let mut text = code.finish().unwrap();
        text.resize(9 * 1024 * 1024, 0);
        fs::write(&binary.0, macho(&text)).unwrap();
        fs::set_permissions(&binary.0, fs::Permissions::from_mode(0o755)).unwrap();
        let verified = Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict"])
            .arg(&binary.0)
            .output()
            .unwrap();
        assert!(
            verified.status.success(),
            "{}",
            String::from_utf8_lossy(&verified.stderr)
        );
        assert_eq!(
            Command::new(&binary.0).env_clear().status().unwrap().code(),
            Some(42)
        );
    }
    #[test]
    fn signing_hash_matches_published_vectors() {
        let hex = |s: &[u8]| {
            sha256(s)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
