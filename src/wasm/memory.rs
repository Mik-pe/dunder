//! Managed aggregate and string operations. The allocation and UTF-8 routines
//! themselves are Dunder code in runtime/wasm.ddr.
use super::*;
use crate::check::{DynamicOp, StringOp};

pub(super) fn builtin(name: &str) -> Result<Body, String> {
    let instructions: &[u8] = match name {
        "cursor" => &[0x23, 0],
        "set_cursor" => &[0x20, 0, 0x24, 0],
        "grow" => &[0x20, 0, 0x40, 0],
        "pages" => &[0x3f, 0],
        "copy" => &[0x20, 0, 0x20, 1, 0x20, 2, 0xfc, 10, 0, 0],
        "fill" => &[0x20, 0, 0x20, 1, 0x20, 2, 0xfc, 11, 0],
        _ => return Err(format!("unknown internal wasm runtime operation '{name}'")),
    };
    let mut bytes = instructions.to_vec();
    bytes.push(0x0b);
    Ok(Body {
        bytes,
        locals: vec![],
        calls: vec![],
    })
}
fn string(ty: Type) -> bool {
    matches!(ty, Type::String | Type::Runes | Type::Bytes)
}

impl Emitter<'_, '_> {
    fn managed(&self, ty: Type) -> bool {
        if string(ty) || self.m.types.dynamic_elem(ty).is_some() {
            return true;
        }
        if let Some(arr) = self.m.types.array(ty) {
            return self.managed(arr.elem);
        }
        if let Some(rec) = self.m.types.record(ty) {
            return rec.fields.iter().any(|f| self.managed(f.ty));
        }
        self.m.types.enumeration(ty).is_some_and(|e| {
            e.variants
                .iter()
                .any(|v| v.fields.iter().any(|f| self.managed(f.ty)))
        })
    }
    pub(super) fn retain(&mut self, ty: Type) {
        if string(ty) {
            self.runtime_call("str_retain");
        }
    }
    pub(super) fn replace_local(&mut self, slot: usize, ty: Type) {
        if string(ty) {
            self.retain(ty);
            self.read(slot, ty);
            self.runtime_call("str_release");
        }
        self.write(slot, ty);
    }
    pub(super) fn drop_slot(&mut self, slot: usize, ty: Type) {
        if string(ty) {
            self.read(slot, ty);
            self.runtime_call("str_release");
            self.int(I32, 0);
            self.write(slot, ty);
        } else if self.managed(ty) {
            self.address(slot);
            let ptr = self.save(I32);
            self.visit_owners(ty, ptr, false);
        }
    }
    pub(super) fn drop_storage(&mut self, slots: &[(usize, Type)]) {
        for &(slot, ty) in slots.iter().rev() {
            self.drop_slot(slot, ty);
        }
    }
    fn child(&mut self, ty: Type, ptr: usize, off: usize, clone: bool) {
        if self.managed(ty) {
            self.get(ptr);
            self.add_offset(off);
            let p = self.save(I32);
            self.visit_owners(ty, p, clone);
        }
    }
    /// Walk just the live reference-bearing fields. A clone operates on a
    /// shallow copy of the source; dropping clears the stored handles.
    fn visit_owners(&mut self, ty: Type, p: usize, clone: bool) {
        if let Some(elem) = self.m.types.dynamic_elem(ty) {
            self.get(p);
            self.int(I32, i64::from(string(elem)));
            self.int(I32, self.m.types.scalar_size(elem) as i64);
            self.runtime_call(if clone { "dyn_clone" } else { "dyn_drop" });
        } else if string(ty) {
            self.get(p);
            self.load_store(Type::Ptr, false, 0);
            if clone {
                self.runtime_call("str_retain");
                self.op(0x1a);
            } else {
                self.runtime_call("str_release");
                self.get(p);
                self.int(I32, 0);
                self.load_store(Type::Ptr, true, 0);
            }
        } else if let Some(a) = self.m.types.array(ty) {
            if a.len == 0 || !self.managed(a.elem) {
                return;
            }
            self.int(I32, a.len as i64);
            let remaining = self.save(I32);
            self.get(p);
            let addr = self.save(I32);
            self.bytes(&[0x03, EMPTY]);
            self.visit_owners(a.elem, addr, clone);
            self.get(addr);
            self.add_offset(a.elem_size());
            self.set(addr);
            self.get(remaining);
            self.int(I32, 1);
            self.op(0x6b);
            self.tee(remaining);
            self.arg(0x0d, 0);
            self.op(0x0b);
        } else if let Some(rec) = self.m.types.record(ty) {
            for f in &rec.fields {
                self.child(f.ty, p, f.offset, clone);
            }
        } else if let Some(en) = self.m.types.enumeration(ty) {
            for v in &en.variants {
                if !v.fields.iter().any(|f| self.managed(f.ty)) {
                    continue;
                }
                self.get(p);
                self.load_store(Type::I32, false, 0);
                self.int(I32, v.tag.into());
                self.op(0x46);
                self.bytes(&[0x04, EMPTY]);
                for f in &v.fields {
                    self.child(f.ty, p, f.offset, clone);
                }
                self.op(0x0b);
            }
        }
    }
    /// Stack: destination, source. Snapshot first so assigning an aggregate to
    /// itself (or from one of its own fields) survives release of its old value.
    pub(super) fn replace(&mut self, ty: Type) {
        let n = self.m.types.storage_bytes(ty).unwrap();
        if !self.managed(ty) {
            self.copy(n);
            return;
        }
        let source = self.save(I32);
        let dst = self.save(I32);
        self.int(I32, n as i64);
        self.runtime_call("alloc");
        let temp = self.save(I32);
        self.get(temp);
        self.get(source);
        self.copy(n);
        self.visit_owners(ty, temp, true);
        self.visit_owners(ty, dst, false);
        self.get(dst);
        self.get(temp);
        self.copy(n);
        self.get(temp);
        self.runtime_call("free");
    }
    pub(super) fn store_value(&mut self, ty: Type) {
        if self.m.types.storage_bytes(ty).is_some() {
            self.replace(ty);
        } else if string(ty) {
            self.retain(ty);
            let value = self.save(I32);
            let dest = self.save(I32);
            self.get(dest);
            self.load_store(ty, false, 0);
            self.runtime_call("str_release");
            self.get(dest);
            self.get(value);
            self.load_store(ty, true, 0);
        } else {
            self.load_store(ty, true, 0);
        }
    }
    fn stride(&self, ty: Type) -> usize {
        self.m
            .types
            .storage_bytes(ty)
            .unwrap_or_else(|| self.m.types.scalar_size(ty))
            .max(1)
    }
    pub(super) fn element_address(&mut self, base: &Expr, i: &Expr) -> Result<(), String> {
        let p = self.eval_saved(base)?;
        let at = self.eval_saved(i)?;
        self.get(at);
        let stride = if let Some(arr) = self.m.types.array(base.ty) {
            self.int(I32, arr.len as i64);
            arr.elem_size()
        } else {
            self.get(p);
            self.load_store(Type::I32, false, 8);
            self.stride(
                self.m
                    .types
                    .dynamic_elem(base.ty)
                    .ok_or("invalid array receiver")?,
            )
        };
        self.op(0x4f);
        self.trap_if();
        self.get(p);
        if self.m.types.dynamic_elem(base.ty).is_some() {
            self.load_store(Type::Ptr, false, 0);
        }
        self.get(at);
        self.int(I32, stride as i64);
        self.op(0x6c);
        self.op(0x6a);
        Ok(())
    }
    pub(super) fn dynamic(&mut self, ty: Type, values: &[Expr], s: usize) -> Result<(), String> {
        let elem = self.m.types.dynamic_elem(ty).unwrap();
        let stride = self.stride(elem);
        self.drop_slot(s, ty);
        if !values.is_empty() {
            self.address(s);
            self.int(I32, values.len() as i64);
            self.int(I32, stride as i64);
            self.runtime_call("dyn_reserve");
            for (i, v) in values.iter().enumerate() {
                self.address(s);
                self.load_store(Type::Ptr, false, 0);
                self.expr(v)?;
                self.retain(elem);
                self.load_store(elem, true, i * stride);
            }
            self.address(s);
            self.int(I32, values.len() as i64);
            self.load_store(Type::U32, true, 8);
        }
        self.address(s);
        Ok(())
    }
    pub(super) fn dynamic_op(
        &mut self,
        op: DynamicOp,
        base: &Expr,
        arg: Option<&Expr>,
    ) -> Result<(), String> {
        let elem = self
            .m
            .types
            .dynamic_elem(base.ty)
            .ok_or("invalid array type")?;
        let stride = self.stride(elem);
        let p = self.eval_saved(base)?;
        match op {
            DynamicOp::Len | DynamicOp::Capacity => {
                self.get(p);
                self.load_store(
                    Type::U32,
                    false,
                    if matches!(op, DynamicOp::Len) { 8 } else { 16 },
                );
            }
            DynamicOp::Reserve => {
                self.get(p);
                self.expr(arg.unwrap())?;
                self.int(I32, stride as i64);
                self.runtime_call("dyn_reserve");
            }
            DynamicOp::Clear => {
                self.get(p);
                self.int(I32, i64::from(string(elem)));
                self.int(I32, stride as i64);
                self.runtime_call("dyn_clear");
            }
            DynamicOp::Push => {
                self.expr(arg.unwrap())?;
                self.retain(elem);
                let value = self.save(val(elem));
                self.get(p);
                self.load_store(Type::U32, false, 8);
                let len = self.save(I32);
                self.get(p);
                self.get(len);
                self.add_offset(1);
                self.int(I32, stride as i64);
                self.runtime_call("dyn_reserve");
                self.get(p);
                self.load_store(Type::Ptr, false, 0);
                self.get(len);
                self.int(I32, stride as i64);
                self.op(0x6c);
                self.op(0x6a);
                self.get(value);
                self.load_store(elem, true, 0);
                self.get(p);
                self.get(len);
                self.add_offset(1);
                self.load_store(Type::U32, true, 8);
            }
            DynamicOp::Pop => {
                self.get(p);
                self.load_store(Type::U32, false, 8);
                let n = self.save(I32);
                self.get(n);
                self.op(0x45);
                self.trap_if();
                self.get(p);
                self.get(n);
                self.int(I32, 1);
                self.op(0x6b);
                self.tee(n);
                self.load_store(Type::U32, true, 8);
                self.get(p);
                self.load_store(Type::Ptr, false, 0);
                self.get(n);
                self.int(I32, stride as i64);
                self.op(0x6c);
                self.op(0x6a);
                self.load_store(elem, false, 0);
            }
        }
        Ok(())
    }
    pub(super) fn scratch(
        &mut self,
        ty: Type,
        area: usize,
        cursor: usize,
        limit: usize,
        count: &Expr,
        s: usize,
    ) -> Result<(), String> {
        let elem = self.m.types.dynamic_elem(ty).unwrap();
        let stride = self.stride(elem);
        let align = self
            .m
            .types
            .record(elem)
            .map(|r| if r.c_layout { r.alignment } else { 8 })
            .or_else(|| self.m.types.array(elem).map(|a| a.elem_size()))
            .unwrap_or(stride);
        let n = self.eval_saved(count)?;
        self.get(n);
        self.int(I32, 0);
        self.op(0x48);
        self.trap_if();
        self.read(cursor, Type::I32);
        self.op(0xad);
        self.int(I64, (align - 1) as i64);
        self.op(0x7c);
        self.int(I64, -(align as i64));
        self.op(0x83);
        let start = self.save(I64);
        self.get(n);
        self.op(0xad);
        self.int(I64, stride as i64);
        self.op(0x7e);
        let size = self.save(I64);
        self.get(start);
        self.get(size);
        self.op(0x7c);
        let end = self.save(I64);
        self.get(end);
        self.read(limit, Type::I32);
        self.op(0xad);
        self.op(0x56);
        self.trap_if();
        self.get(end);
        self.op(0xa7);
        self.write(cursor, Type::I32);
        self.read(area, Type::Ptr);
        self.load_store(Type::Ptr, false, 0);
        self.get(start);
        self.op(0xa7);
        self.op(0x6a);
        let data = self.save(I32);
        self.address(s);
        self.get(data);
        self.load_store(Type::Ptr, true, 0);
        for off in [8, 16] {
            self.address(s);
            self.get(n);
            self.load_store(Type::U32, true, off);
        }
        self.get(data);
        self.int(I32, 0);
        self.get(size);
        self.op(0xa7);
        self.bytes(&[0xfc, 11, 0]);
        self.address(s);
        Ok(())
    }
    pub(super) fn string_method(&mut self, op: StringOp, args: &[Expr]) -> Result<(), String> {
        for a in args {
            self.expr(a)?;
        }
        match op {
            StringOp::Len => self.runtime_call("strlen"),
            StringOp::RuneLen => {
                self.int(I32, 0);
                self.int(I32, 1);
                self.runtime_call("runes");
            }
            StringOp::RuneAt => {
                self.int(I32, 0);
                self.runtime_call("runes");
            }
            StringOp::Slice => {
                let range = self.save(I64);
                self.get(range);
                self.op(0xa7);
                self.get(range);
                self.int(I64, 32);
                self.op(0x88);
                self.op(0xa7);
                self.runtime_call("str_slice");
            }
            StringOp::ByteAt => {
                let at = self.save(I32);
                let p = self.save(I32);
                self.get(at);
                self.get(p);
                self.runtime_call("strlen");
                self.op(0x4f);
                self.trap_if();
                self.get(p);
                self.get(at);
                self.op(0x6a);
                self.load_store(Type::U8, false, 8);
            }
            StringOp::ToRune => {
                if val(args[0].ty) == I64 {
                    let n = self.save(I64);
                    self.get(n);
                    self.int(I64, 0x10ffff);
                    self.op(0x56);
                    self.trap_if();
                    self.get(n);
                    self.op(0xa7);
                }
                let n = self.save(I32);
                self.get(n);
                self.int(I32, 0x10ffff);
                self.op(0x4b);
                self.trap_if();
                self.get(n);
                self.int(I32, 0xd800);
                self.op(0x4f);
                self.get(n);
                self.int(I32, 0xdfff);
                self.op(0x4d);
                self.op(0x71);
                self.trap_if();
                self.get(n);
            }
            StringOp::ToString => {
                let ty = args[0].ty;
                if ty == Type::Bool {
                    self.bytes(&[0x04, I32]);
                    let yes = self.m.literal("true");
                    self.int(I32, yes as i64);
                    self.op(0x05);
                    let no = self.m.literal("false");
                    self.int(I32, no as i64);
                    self.op(0x0b);
                } else if ty == Type::Rune {
                    self.runtime_call("str_rune");
                } else if ty.float() {
                    if ty == Type::F32 {
                        self.op(0xbc);
                        self.op(0xad);
                    } else {
                        self.op(0xbd);
                    }
                    self.int(I32, i64::from(ty == Type::F32));
                    self.m.needs_formatter = true;
                    self.direct_call(self.m.formatter.ok_or("missing float formatter")?);
                } else if ty != Type::String {
                    if val(ty) == I32 {
                        self.op(if ty.unsigned() { 0xad } else { 0xac });
                    }
                    let n = self.save(I64);
                    self.get(n);
                    if ty.unsigned() {
                        self.int(I32, 0);
                    } else {
                        self.get(n);
                        self.int(I64, 0);
                        self.op(0x53);
                    }
                    self.runtime_call("str_int");
                }
            }
        }
        Ok(())
    }
}
