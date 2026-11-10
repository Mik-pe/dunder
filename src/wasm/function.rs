use super::*;
use crate::check::{EnumArm, ExprKind as E, Stmt as S};

pub(super) fn compile(m: &mut Module<'_>, f: &Function) -> Result<Body, String> {
    let params = f.params.len() + usize::from(f.return_slot.is_some());
    let mut taken = HashSet::new();
    let mut frame_required = f.param_storage.iter().any(Option::is_some);
    loop {
        let mut c = Emitter {
            m,
            f,
            body: Body {
                bytes: vec![],
                locals: vec![],
                calls: vec![],
            },
            locals: BTreeMap::new(),
            params,
            frame: params,
            frame_used: false,
            frame_required,
            address_taken: taken.clone(),
            depth: 0,
            loops: vec![],
        };
        c.tmp(I32); // saved incoming stack pointer, also the frame base
        c.stmts(&f.body)?;
        let needs_frame = c.frame_used || frame_required;
        if c.address_taken != taken || needs_frame != frame_required {
            // LocalAddress may follow a read or write. Re-emit once with the
            // complete spill set so those earlier accesses see the same cell.
            frame_required = needs_frame;
            taken = c.address_taken;
            continue;
        }
        if f.ret != Type::Unit {
            c.op(0x00);
        } else {
            c.restore();
        }
        c.op(0x0b);
        let body = std::mem::take(&mut c.body.bytes);
        let mut calls = std::mem::take(&mut c.body.calls);
        if frame_required {
            if c.m.emitting_runtime {
                return Err("internal Wasm runtime requires a scalar-only allocator".into());
            }
            let n = f
                .slots
                .checked_mul(8)
                .filter(|n| *n <= i32::MAX as usize)
                .ok_or("frame too large")?;
            c.int(I32, n as i64);
            c.runtime_call("alloc");
            c.tee(c.frame);
            c.int(I32, 0);
            c.int(I32, n as i64);
            c.bytes(&[0xfc, 11, 0]);
        }
        for (i, &slot) in f.param_slots.iter().enumerate() {
            let ty = f.params[i];
            if let Some(storage) = f.param_storage[i] {
                c.address(storage);
                c.get(i);
                c.replace(ty);
                c.address(storage);
            } else {
                c.get(i);
                c.retain(ty);
            }
            c.write(slot, ty);
        }
        for (pos, _) in &mut calls {
            *pos += c.body.bytes.len();
        }
        c.body.calls.extend(calls);
        c.body.bytes.extend(body);
        return Ok(c.body);
    }
}
impl Emitter<'_, '_> {
    fn restore(&mut self) {
        self.drop_storage(&self.f.owned_storage);
        if self.frame_required {
            self.get(self.frame);
            self.runtime_call("free");
        }
    }
    pub(super) fn expr(&mut self, e: &Expr) -> Result<(), String> {
        match &e.kind {
            E::Number(n) => self.int(val(e.ty), *n),
            E::Bool(b) => self.int(I32, i64::from(*b)),
            E::Float(bits) if e.ty == Type::F32 => {
                self.op(0x43);
                self.bytes(&(*bits as u32).to_le_bytes());
            }
            E::Float(bits) => {
                self.op(0x44);
                self.bytes(&bits.to_le_bytes());
            }
            E::Local(slot) => self.read(*slot, e.ty),
            E::ReferenceAddress(slot) => self.read(*slot, Type::Ptr),
            E::LocalAddress(slot) => {
                self.address_taken.insert(*slot);
                self.address(*slot);
            }
            E::Function(id) | E::Callback(id) => self.int(I32, self.m.table[*id] as i64),
            E::Cast(a) => {
                self.expr(a)?;
                self.cast(a.ty, e.ty)?;
            }
            E::Binary(op, a, b) => self.binary(op, a, b)?,
            E::IntOp(op, a, b) => self.integer_method(*op, a, b.as_deref())?,
            E::FloatOp(op, a) => self.float_method(*op, a, e.ty)?,
            E::Unary(op, a) => {
                self.expr(a)?;
                if *op == "!" {
                    self.op(0x45);
                } else if a.ty.float() {
                    self.op(if a.ty == Type::F64 { 0x9a } else { 0x8c });
                } else {
                    let ty = val(a.ty);
                    let t = self.save(ty);
                    self.get(t);
                    self.int(
                        ty,
                        if ty == I64 {
                            i64::MIN
                        } else {
                            i64::from(i32::MIN)
                        },
                    );
                    self.op(if ty == I64 { 0x51 } else { 0x46 });
                    self.trap_if();
                    self.int(ty, 0);
                    self.get(t);
                    self.op(if ty == I64 { 0x7d } else { 0x6b });
                }
            }
            E::Call(callee, args, storage) => {
                let direct = if let E::Function(id) = callee.kind {
                    Some(id)
                } else {
                    None
                };
                let pointer = if direct.is_none() {
                    Some(self.eval_saved(callee)?)
                } else {
                    None
                };
                for a in args {
                    self.expr(a)?;
                }
                if let Some(storage) = storage {
                    self.address(*storage);
                }
                if let Some(id) = direct {
                    self.direct_call(id);
                } else {
                    let f = self
                        .m
                        .types
                        .signature(callee.ty)
                        .ok_or("invalid indirect function type")?;
                    let ty = self
                        .m
                        .signature(Signature::new(self.m.types, &f.params, f.ret));
                    self.get(pointer.unwrap());
                    self.arg(0x11, ty);
                    self.op(0);
                }
                if let Some(storage) = storage {
                    self.address(*storage);
                }
                // Foreign functions may supply high bits for narrow values.
                if self.m.types.is_extern_fn(callee.ty) {
                    if let Some(max) = Self::unsigned_limit(e.ty) {
                        self.int(I32, max.into());
                        self.op(0x71);
                    }
                }
            }
            E::Assert(a) => {
                self.expr(a)?;
                self.op(0x45);
                self.trap_if();
            }
            E::Print(a) => {
                self.expr(a)?;
                let name = match a.ty {
                    Type::String => "print_string",
                    Type::Bool => "print_bool",
                    Type::Rune => "print_rune",
                    Type::I64 => "print_i64",
                    Type::F32 => "print_f32",
                    Type::F64 => "print_f64",
                    Type::U64 => "print_u64",
                    t if t.unsigned() => "print_u32",
                    _ => "print_i32",
                };
                let sig = self.m.signature(Signature {
                    args: vec![val(a.ty)],
                    result: None,
                });
                let i = self.m.import("dunder", name, sig);
                self.arg(0x10, i);
            }
            E::ManagedString(a, slot, fresh) => {
                self.expr(a)?;
                if !fresh {
                    self.retain(e.ty);
                }
                self.read(*slot, e.ty);
                self.runtime_call("str_release");
                self.write(*slot, e.ty);
                self.read(*slot, e.ty);
            }
            E::Iterator(a) | E::StringView(a) => self.expr(a)?,
            E::String(s) => {
                let p = self.m.literal(s);
                self.int(I32, p as i64);
            }
            E::StringOp(op, args) => self.string_method(*op, args)?,
            E::Range(a, b) => {
                self.expr(a)?;
                self.op(0xad);
                self.expr(b)?;
                self.op(0xad);
                self.int(I64, 32);
                self.op(0x86);
                self.op(0x84);
            }
            E::Len(a, n) => {
                self.expr(a)?;
                self.op(0x1a);
                self.int(I32, *n as i64);
            }
            E::Clone(a, s) => {
                self.address(*s);
                self.expr(a)?;
                self.replace(e.ty);
                self.address(*s);
            }
            E::Deref(a) => {
                self.expr(a)?;
                if self.m.types.storage_bytes(e.ty).is_none() {
                    self.load_store(e.ty, false, 0);
                }
            }
            E::Field(a, off) | E::Offset(a, off) => {
                self.expr(a)?;
                if matches!(&e.kind, E::Field(..)) && self.m.types.storage_bytes(e.ty).is_none() {
                    self.load_store(e.ty, false, *off);
                } else {
                    self.add_offset(*off);
                }
            }
            E::EnumFromInt(a) => {
                self.expr(a)?;
                let t = self.save(I32);
                self.int(I32, 0);
                for v in &self.m.types.enumeration(e.ty).unwrap().variants {
                    self.get(t);
                    self.int(I32, v.tag.into());
                    self.op(0x46);
                    self.op(0x72);
                }
                self.op(0x45);
                self.trap_if();
                self.get(t);
            }
            E::Enum {
                tag,
                fields,
                storage,
            } => {
                if let Some(storage) = storage {
                    self.record(e.ty, *storage, fields)?;
                    self.address(*storage);
                    self.int(I32, i64::from(*tag));
                    self.load_store(Type::I32, true, 0);
                } else {
                    self.int(I32, (*tag).into());
                }
            }
            E::Record(fields, s) => self.record(e.ty, *s, fields)?,
            E::ElementAddress(a, i) | E::Index(a, i) => {
                self.element_address(a, i)?;
                if matches!(e.kind, E::Index(..)) && self.m.types.storage_bytes(e.ty).is_none() {
                    self.load_store(e.ty, false, 0);
                }
            }
            E::Array(values, s) => {
                let arr = self.m.types.array(e.ty).unwrap();
                self.drop_slot(*s, e.ty);
                for (i, v) in values.iter().enumerate() {
                    self.address(*s);
                    self.expr(v)?;
                    self.retain(arr.elem);
                    self.load_store(arr.elem, true, i * arr.elem_size());
                }
                self.address(*s);
            }
            E::Repeat(v, s) => {
                let arr = self.m.types.array(e.ty).unwrap();
                let value = self.eval_saved(v)?;
                self.drop_slot(*s, e.ty);
                if arr.len != 0 {
                    self.address(*s);
                    let data = self.save(I32);
                    self.int(I32, arr.len as i64);
                    let remain = self.save(I32);
                    self.bytes(&[0x03, EMPTY]);
                    self.get(data);
                    self.get(value);
                    self.retain(arr.elem);
                    self.load_store(arr.elem, true, 0);
                    self.get(data);
                    self.add_offset(arr.elem_size());
                    self.set(data);
                    self.get(remain);
                    self.int(I32, 1);
                    self.op(0x6b);
                    self.tee(remain);
                    self.arg(0x0d, 0);
                    self.op(0x0b);
                }
                self.address(*s);
            }
            E::SysCall(crate::check::SysOp::DecodeUtf8, args, Some(storage)) => {
                self.drop_slot(*storage, e.ty);
                let record = self.m.types.record(e.ty).unwrap();
                let offset = |name: &str| {
                    record
                        .fields
                        .iter()
                        .find(|f| f.name == name)
                        .unwrap()
                        .offset
                };
                let (ok, value, error) = (offset("ok"), offset("value"), offset("error"));
                let empty = self.m.literal("");
                self.address(*storage);
                self.int(I32, empty as i64);
                self.load_store(Type::Ptr, true, value);
                self.expr(&args[0])?;
                let array = self.m.types.value_type(args[0].ty);
                self.int(
                    I32,
                    i64::from(self.m.types.dynamic_elem(array) == Some(Type::U8)),
                );
                self.address(*storage);
                self.add_offset(value);
                self.runtime_call("decode_utf8");
                let err = self.save(I32);
                self.address(*storage);
                self.get(err);
                self.load_store(Type::I32, true, error);
                self.address(*storage);
                self.get(err);
                self.op(0x45);
                self.load_store(Type::Bool, true, ok);
                self.address(*storage);
            }
            E::SysCall(..) => {
                return Err("native system calls require an explicit wasm host import".into())
            }
            E::StaticBytes { .. } => {
                return Err("native static storage requires a native target".into())
            }
            E::Dynamic(values, s) => self.dynamic(e.ty, values, *s)?,
            E::DynamicOp(op, base, arg) => self.dynamic_op(*op, base, arg.as_deref())?,
            E::ScratchAlloc {
                area,
                cursor,
                limit,
                count,
                storage,
            } => self.scratch(e.ty, *area, *cursor, *limit, count, *storage)?,
        }
        Ok(())
    }
    pub(super) fn add_offset(&mut self, n: usize) {
        if n != 0 {
            self.int(I32, n as i64);
            self.op(0x6a);
        }
    }
    fn record(&mut self, ty: Type, s: usize, fields: &[(usize, Expr)]) -> Result<(), String> {
        self.drop_slot(s, ty);
        self.address(s);
        self.int(I32, 0);
        self.int(I32, self.m.types.storage_bytes(ty).unwrap() as i64);
        self.bytes(&[0xfc, 11, 0]);
        for (off, v) in fields {
            self.address(s);
            if self.m.types.storage_bytes(v.ty).is_some() {
                self.add_offset(*off);
                self.expr(v)?;
                self.replace(v.ty);
            } else {
                self.expr(v)?;
                self.retain(v.ty);
                self.load_store(v.ty, true, *off);
            }
        }
        self.address(s);
        Ok(())
    }
    fn stmts(&mut self, body: &[S]) -> Result<(), String> {
        for s in body {
            match s {
                S::Return(e) => {
                    if let Some(e) = e {
                        if self.m.types.storage_bytes(e.ty).is_some() {
                            self.get(self.f.params.len());
                            self.expr(e)?;
                            self.replace(e.ty);
                        } else {
                            self.expr(e)?;
                            self.retain(e.ty);
                        }
                    }
                    self.restore();
                    self.op(0x0f);
                }
                S::Store(slot, e) => {
                    self.expr(e)?;
                    self.replace_local(*slot, e.ty);
                }
                S::StoreAggregate(slot, s, e) => {
                    self.address(*s);
                    self.expr(e)?;
                    self.replace(e.ty);
                    self.address(*s);
                    self.write(*slot, e.ty);
                }
                S::StorePlace(a, v) => {
                    self.expr(a)?;
                    self.expr(v)?;
                    self.store_value(v.ty);
                }
                S::Expr(e) => {
                    self.expr(e)?;
                    if e.ty != Type::Unit {
                        self.op(0x1a);
                    }
                }
                S::Block(b) => self.stmts(b)?,
                S::DropStorage(slots) => self.drop_storage(slots),
                S::If(c, a, b) => {
                    self.expr(c)?;
                    self.begin(0x04);
                    self.stmts(a)?;
                    if !b.is_empty() {
                        self.op(0x05);
                        self.stmts(b)?;
                    }
                    self.end();
                }
                S::While(c, b) => {
                    let done = self.begin(0x02);
                    let head = self.begin(0x03);
                    self.expr(c)?;
                    self.op(0x45);
                    self.branch(0x0d, done);
                    self.loops.push((done, head));
                    self.stmts(b)?;
                    self.loops.pop();
                    self.branch(0x0c, head);
                    self.end();
                    self.end();
                }
                S::For {
                    source,
                    storage,
                    binding,
                    body,
                    ..
                } => self.for_loop(source, *storage, *binding, body)?,
                S::Match {
                    source,
                    storage,
                    arms,
                    ..
                } => self.match_enum(source, *storage, arms)?,
                S::Break | S::Continue => {
                    let (done, next) = self.loops.last().unwrap();
                    self.branch(0x0c, if matches!(s, S::Break) { *done } else { *next });
                }
            }
        }
        Ok(())
    }
    fn for_loop(
        &mut self,
        source: &Expr,
        storage: Option<usize>,
        binding: usize,
        body: &[S],
    ) -> Result<(), String> {
        let aggregate = self.m.types.storage_bytes(source.ty);
        if let (Some(s), Some(_)) = (storage, aggregate) {
            self.address(s);
            self.expr(source)?;
            self.replace(source.ty);
            self.address(s);
        } else {
            self.expr(source)?;
        }
        let base = self.save(val(source.ty));
        let cursor = self.tmp(I32);
        let end = self.tmp(I32);
        let element = if source.ty == Type::Range {
            self.get(base);
            self.op(0xa7);
            self.set(cursor);
            self.get(base);
            self.int(I64, 32);
            self.op(0x88);
            self.op(0xa7);
            None
        } else {
            self.int(I32, 0);
            self.set(cursor);
            if let Some(arr) = self.m.types.array(source.ty) {
                self.int(I32, arr.len as i64);
                Some((arr.elem, arr.elem_size(), 0))
            } else if let Some(elem) = self.m.types.dynamic_elem(source.ty) {
                self.get(base);
                self.load_store(Type::I32, false, 8);
                Some((elem, self.m.types.scalar_size(elem), 0))
            } else if matches!(source.ty, Type::Bytes | Type::Runes) {
                self.get(base);
                self.load_store(Type::I32, false, 0);
                Some((Type::U8, 1, 8))
            } else {
                return Err("this iterator requires the native runtime".into());
            }
        };
        self.set(end);
        let done = self.begin(0x02);
        let head = self.begin(0x03);
        self.get(cursor);
        self.get(end);
        self.op(0x4e);
        self.branch(0x0d, done);
        if source.ty == Type::Runes {
            self.get(base);
            self.get(cursor);
            self.runtime_call("rune_at_byte");
            self.write(binding, Type::Rune);
        } else if let Some((ty, stride, off)) = element {
            self.get(base);
            if self.m.types.dynamic_elem(source.ty).is_some() {
                self.load_store(Type::Ptr, false, 0);
            }
            self.get(cursor);
            self.int(I32, stride as i64);
            self.op(0x6c);
            self.op(0x6a);
            self.load_store(ty, false, off);
            self.replace_local(binding, ty);
        } else {
            self.get(cursor);
            self.write(binding, Type::I32);
        }
        let next = self.begin(0x02);
        self.loops.push((done, next));
        self.stmts(body)?;
        self.loops.pop();
        self.end();
        self.get(cursor);
        if source.ty == Type::Runes {
            self.get(base);
            self.get(cursor);
            self.op(0x6a);
            self.load_store(Type::U8, false, 8);
            self.runtime_call("rune_width");
        } else {
            self.int(I32, 1);
        }
        self.op(0x6a);
        self.set(cursor);
        self.branch(0x0c, head);
        self.end();
        self.end();
        if let Some((ty, _, _)) = element {
            if ty == Type::String {
                self.drop_slot(binding, ty);
            }
        }
        if let Some(s) = storage {
            self.drop_slot(s, source.ty);
        }
        Ok(())
    }
    fn match_enum(
        &mut self,
        source: &Expr,
        storage: Option<usize>,
        arms: &[EnumArm],
    ) -> Result<(), String> {
        if let Some(s) = storage {
            self.address(s);
            self.expr(source)?;
            self.replace(source.ty);
            self.address(s);
        } else {
            self.expr(source)?;
        }
        let base = self.save(I32);
        self.get(base);
        if storage.is_some() {
            self.load_store(Type::I32, false, 0);
        }
        let tag = self.save(I32);
        let done = self.begin(0x02);
        for arm in arms {
            if let Some(value) = arm.tag {
                self.get(tag);
                self.int(I32, value.into());
                self.op(0x46);
                self.begin(0x04);
            }
            for bind in &arm.bindings {
                if let Some(s) = bind.storage {
                    self.address(s);
                    self.get(base);
                    self.add_offset(bind.offset);
                    self.replace(bind.ty);
                    self.address(s);
                } else {
                    self.get(base);
                    self.load_store(bind.ty, false, bind.offset);
                }
                self.replace_local(bind.slot, bind.ty);
            }
            self.stmts(&arm.body)?;
            self.branch(0x0c, done);
            if arm.tag.is_some() {
                self.end();
            }
        }
        self.op(0x00);
        self.end();
        Ok(())
    }
}
