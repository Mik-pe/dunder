use super::*;
use crate::check::{EnumArm, ExprKind as E, Stmt as S, StringOp};

pub(super) fn compile(m: &mut Module<'_>, f: &Function) -> Result<Body, String> {
    let params = f.params.len() + usize::from(f.return_slot.is_some());
    let mut taken = HashSet::new();
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
            address_taken: taken.clone(),
            depth: 0,
            loops: vec![],
        };
        c.tmp(I32); // saved incoming stack pointer, also the frame base
        c.stmts(&f.body)?;
        if c.address_taken != taken {
            // LocalAddress may follow a read or write. Re-emit once with the
            // complete spill set so those earlier accesses see the same cell.
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
        let frame = c.frame_used || f.param_storage.iter().any(Option::is_some);
        c.arg(0x23, 0);
        c.set(c.frame);
        if frame {
            c.reserve_frame()?;
        }
        for (i, &slot) in f.param_slots.iter().enumerate() {
            let ty = f.params[i];
            if let Some(storage) = f.param_storage[i] {
                c.address(storage);
                c.get(i);
                c.copy(c.m.types.storage_bytes(ty).unwrap());
                c.address(storage);
            } else {
                c.get(i);
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
    fn reserve_frame(&mut self) -> Result<(), String> {
        let size = self
            .f
            .slots
            .checked_mul(8)
            .filter(|n| *n <= i32::MAX as usize)
            .ok_or("frame too large")?;
        self.get(self.frame);
        self.int(I32, size as i64);
        self.op(0x6a);
        let top = self.save(I32);
        self.get(top);
        self.get(self.frame);
        self.op(0x49);
        self.trap_if();
        self.get(top);
        self.int(I32, 1);
        self.op(0x6b);
        self.int(I32, 16);
        self.op(0x76);
        self.int(I32, 1);
        self.op(0x6a);
        let needed = self.save(I32);
        self.get(needed);
        self.bytes(&[0x3f, 0, 0x4b, 0x04, EMPTY]);
        self.get(needed);
        self.bytes(&[0x3f, 0, 0x6b, 0x40, 0]);
        self.int(I32, -1);
        self.op(0x46);
        self.trap_if();
        self.op(0x0b);
        self.get(top);
        self.arg(0x24, 0);
        Ok(())
    }
    fn restore(&mut self) {
        self.get(self.frame);
        self.arg(0x24, 0);
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
            E::ManagedString(a, _, _) | E::Iterator(a) | E::StringView(a) => self.expr(a)?,
            E::String(s) => {
                let p = self.m.literal(s);
                self.int(I32, p as i64);
            }
            E::StringOp(StringOp::Len, args) => {
                self.expr(&args[0])?;
                self.load_store(Type::U32, false, 0);
            }
            E::StringOp(StringOp::ByteAt, args) => {
                let base = self.eval_saved(&args[0])?;
                let at = self.eval_saved(&args[1])?;
                self.get(at);
                self.get(base);
                self.load_store(Type::U32, false, 0);
                self.op(0x4f);
                self.trap_if();
                self.get(base);
                self.get(at);
                self.op(0x6a);
                self.load_store(Type::U8, false, 8);
            }
            E::StringOp(_, _) => {
                return Err("this string operation requires the native string runtime".into())
            }
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
                self.copy(self.m.types.storage_bytes(e.ty).unwrap());
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
                let arr = self
                    .m
                    .types
                    .array(a.ty)
                    .ok_or("dynamic indexing requires the collection runtime")?;
                let base = self.eval_saved(a)?;
                let at = self.eval_saved(i)?;
                self.get(at);
                self.int(I32, arr.len as i64);
                self.op(0x4f);
                self.trap_if();
                self.get(base);
                self.get(at);
                self.int(I32, arr.elem_size() as i64);
                self.op(0x6c);
                self.op(0x6a);
                if matches!(&e.kind, E::Index(..)) {
                    self.load_store(e.ty, false, 0);
                }
            }
            E::Array(values, s) => {
                let arr = self.m.types.array(e.ty).unwrap();
                for (i, v) in values.iter().enumerate() {
                    self.address(*s);
                    self.expr(v)?;
                    self.load_store(arr.elem, true, i * arr.elem_size());
                }
                self.address(*s);
            }
            E::Repeat(v, s) => {
                let arr = self.m.types.array(e.ty).unwrap();
                let value = self.eval_saved(v)?;
                if arr.len != 0 {
                    self.address(*s);
                    let data = self.save(I32);
                    self.int(I32, arr.len as i64);
                    let remain = self.save(I32);
                    self.bytes(&[0x03, EMPTY]);
                    self.get(data);
                    self.get(value);
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
            E::SysCall(..) => {
                return Err("native system calls require an explicit wasm host import".into())
            }
            E::StaticBytes { .. } => return Err("native static storage requires a native target".into()),
            E::Dynamic(..) | E::DynamicOp(..) => {
                return Err("dynamic arrays require the native collection runtime".into())
            }
            #[allow(unreachable_patterns)]
            _ => return Err("unsupported expression for wasm32".into()),
        }
        Ok(())
    }
    fn add_offset(&mut self, n: usize) {
        if n != 0 {
            self.int(I32, n as i64);
            self.op(0x6a);
        }
    }
    fn record(&mut self, ty: Type, s: usize, fields: &[(usize, Expr)]) -> Result<(), String> {
        self.address(s);
        self.int(I32, 0);
        self.int(I32, self.m.types.storage_bytes(ty).unwrap() as i64);
        self.bytes(&[0xfc, 11, 0]);
        for (off, v) in fields {
            self.address(s);
            if let Some(n) = self.m.types.storage_bytes(v.ty) {
                self.add_offset(*off);
                self.expr(v)?;
                self.copy(n);
            } else {
                self.expr(v)?;
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
                        if let Some(n) = self.m.types.storage_bytes(e.ty) {
                            self.get(self.f.params.len());
                            self.expr(e)?;
                            self.copy(n);
                        } else {
                            self.expr(e)?;
                        }
                    }
                    self.restore();
                    self.op(0x0f);
                }
                S::Store(slot, e) => {
                    self.expr(e)?;
                    self.write(*slot, e.ty);
                }
                S::StoreAggregate(slot, s, e) => {
                    self.address(*s);
                    self.expr(e)?;
                    self.copy(self.m.types.storage_bytes(e.ty).unwrap());
                    self.address(*s);
                    self.write(*slot, e.ty);
                }
                S::StorePlace(a, v) => {
                    self.expr(a)?;
                    self.expr(v)?;
                    if let Some(n) = self.m.types.storage_bytes(v.ty) {
                        self.copy(n);
                    } else {
                        self.load_store(v.ty, true, 0);
                    }
                }
                S::Expr(e) => {
                    self.expr(e)?;
                    if e.ty != Type::Unit {
                        self.op(0x1a);
                    }
                }
                S::Block(b) => self.stmts(b)?,
                S::DropStorage(_) => {}
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
        if let (Some(s), Some(n)) = (storage, aggregate) {
            self.address(s);
            self.expr(source)?;
            self.copy(n);
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
            } else if source.ty == Type::Bytes {
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
        if let Some((ty, stride, off)) = element {
            self.get(base);
            self.get(cursor);
            self.int(I32, stride as i64);
            self.op(0x6c);
            self.op(0x6a);
            self.load_store(ty, false, off);
            self.write(binding, ty);
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
        self.int(I32, 1);
        self.op(0x6a);
        self.set(cursor);
        self.branch(0x0c, head);
        self.end();
        self.end();
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
            self.copy(self.m.types.storage_bytes(source.ty).unwrap());
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
                    self.copy(self.m.types.storage_bytes(bind.ty).unwrap());
                    self.address(s);
                } else {
                    self.get(base);
                    self.load_store(bind.ty, false, bind.offset);
                }
                self.write(bind.slot, bind.ty);
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
