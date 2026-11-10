//! Checking for unsafe operations. Raw addresses carry neither ownership nor a borrow.
use super::*;

pub(super) fn scalar(name: &str) -> Option<Type> {
    Some(match name {
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "usize" => Type::Usize,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "f32" => Type::F32,
        "f64" => Type::F64,
        "ptr" => Type::Ptr,
        _ => return None,
    })
}

pub(super) fn signature(
    types: &Types,
    params: &[Type],
    ret: Type,
    span: Span,
) -> Result<(), Error> {
    let allowed = |ty: Type| {
        ty.numeric()
            || matches!(ty, Type::Ptr | Type::ExternFunction(_))
            || types.homogeneous_float(ty).is_some()
    };
    if params.iter().all(|ty| allowed(*ty)) && (ret == Type::Unit || allowed(ret)) {
        Ok(())
    } else {
        Err(Error::new("E0291", span, "C calls take numeric scalars, ptr, extern fn, and homogeneous C float records; pass other records by pointer"))
    }
}

pub(super) fn validate_type(types: &Types, ty: Type, span: Span) -> Result<(), Error> {
    if let Some(sig) = types.signature(ty) {
        if types.is_extern_fn(ty) {
            signature(types, &sig.params, sig.ret, span)?;
        }
        for param in &sig.params {
            validate_type(types, *param, span)?;
        }
        validate_type(types, sig.ret, span)?;
    } else if let Some((inner, _)) = types.referenced(ty) {
        validate_type(types, inner, span)?;
    }
    Ok(())
}

impl Checker<'_> {
    pub(super) fn require_unsafe(&self, span: Span) -> Result<(), Error> {
        if self.in_unsafe {
            Ok(())
        } else {
            Err(Error::new(
                "E0290",
                span,
                "this operation requires an unsafe function or block",
            ))
        }
    }

    fn zeroed_c_value(&mut self, ty: Type) -> Expr {
        let kind = if let Some(record) = self.types.record(ty).cloned() {
            let fields = record
                .fields
                .iter()
                .map(|f| (f.offset, self.zeroed_c_value(f.ty)))
                .collect();
            ExprKind::Record(fields, self.storage(ty).unwrap())
        } else if let Some(array) = self.types.array(ty) {
            let element = self.zeroed_c_value(array.elem);
            ExprKind::Repeat(Box::new(element), self.storage(ty).unwrap())
        } else if ty.float() {
            ExprKind::Float(0)
        } else {
            ExprKind::Number(0)
        };
        Expr { ty, kind }
    }

    pub(super) fn raw_call(
        &mut self,
        name: &str,
        args: &[ast::Expr],
        expected: Option<Type>,
        span: Span,
    ) -> Result<Expr, Error> {
        use ast::ExprKind as A;
        if name == "zeroed" {
            self.require_unsafe(span)?;
            if !args.is_empty() {
                return Err(Error::new("E0207", span, "zeroed expects no arguments"));
            }
            let ty = expected
                .filter(|t| self.types.record(*t).is_some_and(|r| r.c_layout))
                .ok_or_else(|| {
                    Error::new(
                        "E0291",
                        span,
                        "zeroed() needs an explicit C-layout struct type",
                    )
                })?;
            return Ok(self.zeroed_c_value(ty));
        }
        let [arg] = args else {
            return Err(Error::new(
                "E0207",
                span,
                format!("{name} expects one argument"),
            ));
        };
        if matches!(name, "size_of" | "align_of") {
            let ty = if let A::Name(n) = &arg.kind {
                if let Some(ty) = scalar(n) {
                    ty
                } else {
                    self.constructor_type(arg)?
                }
            } else {
                self.constructor_type(arg)?
            };
            let bytes = if let Some(record) = self.types.record(ty).filter(|r| r.c_layout) {
                if name == "size_of" {
                    record.bytes
                } else {
                    record.alignment
                }
            } else if ty.numeric() || ty == Type::Ptr {
                self.types.scalar_size(ty)
            } else {
                return Err(Error::new(
                    "E0291",
                    arg.span,
                    "use a C-layout struct or scalar type",
                ));
            };
            return Ok(Expr {
                ty: Type::Usize,
                kind: ExprKind::Number(bytes as i64),
            });
        }
        self.require_unsafe(span)?;
        let result = match name {
            "callback" => {
                let f = self.expr(arg, None)?;
                let ExprKind::Function(index) = f.kind else {
                    return Err(Error::new(
                        "E0291",
                        arg.span,
                        "callback requires a named Dunder function",
                    ));
                };
                if !matches!(f.ty, Type::Function(_) | Type::UnsafeFunction(_)) {
                    return Err(Error::new(
                        "E0291",
                        arg.span,
                        "callback requires a Dunder function",
                    ));
                }
                let sig = self.types.signature(f.ty).unwrap();
                signature(self.types, &sig.params, sig.ret, arg.span)?;
                if sig
                    .params
                    .iter()
                    .chain(std::iter::once(&sig.ret))
                    .any(|t| self.types.homogeneous_float(*t).is_some())
                {
                    return Err(Error::new("E0291", arg.span, "C callbacks currently accept and return scalar values; pass records by pointer"));
                }
                let (Type::Function(id) | Type::UnsafeFunction(id)) = f.ty else {
                    unreachable!()
                };
                Expr {
                    ty: Type::ExternFunction(id),
                    kind: ExprKind::Callback(index),
                }
            }
            "addr" => {
                let A::Borrow(place, mutable) = &arg.kind else {
                    return Err(Error::new(
                        "E0290",
                        arg.span,
                        "addr expects &place or &mut place",
                    ));
                };
                self.check_scratch_borrow(place)?;
                let (_, mut address) = self.place(place, *mutable)?;
                address.ty = Type::Ptr;
                address
            }
            "cstr" => {
                let A::String(s) = &arg.kind else {
                    return Err(Error::new(
                        "E0290",
                        arg.span,
                        "cstr expects a string literal",
                    ));
                };
                if s.contains('\0') {
                    return Err(Error::new(
                        "E0290",
                        arg.span,
                        "C string literal contains an embedded NUL",
                    ));
                }
                let literal = Expr {
                    ty: Type::String,
                    kind: ExprKind::String(s.clone()),
                };
                Expr {
                    ty: Type::Ptr,
                    kind: ExprKind::Offset(Box::new(literal), 8),
                }
            }
            "ptr" => {
                let value = self.expr(arg, Some(Type::Usize))?;
                Expr {
                    ty: Type::Ptr,
                    kind: ExprKind::Cast(Box::new(value)),
                }
            }
            "ffi_fn" | "ref_at" => {
                let ty = expected
                    .filter(|t| {
                        if name == "ffi_fn" {
                            self.types.is_extern_fn(*t)
                        } else {
                            self.types.referenced(*t).is_some()
                        }
                    })
                    .ok_or_else(|| {
                        Error::new(
                            "E0290",
                            span,
                            if name == "ffi_fn" {
                                "ffi_fn needs an explicit extern fn type"
                            } else {
                                "ref_at needs a reference parameter type"
                            },
                        )
                    })?;
                let value = self.expr(arg, Some(Type::Ptr))?;
                Expr {
                    ty,
                    kind: ExprKind::Cast(Box::new(value)),
                }
            }
            _ => {
                let ty = name
                    .strip_prefix("load_")
                    .and_then(scalar)
                    .expect("checked load builtin");
                let p = self.expr(arg, Some(Type::Ptr))?;
                Expr {
                    ty,
                    kind: ExprKind::Deref(Box::new(p)),
                }
            }
        };
        Ok(result)
    }

    pub(super) fn raw_store(
        &mut self,
        name: &str,
        args: &[ast::Expr],
        span: Span,
    ) -> Result<Stmt, Error> {
        self.require_unsafe(span)?;
        let [address, value] = args else {
            return Err(Error::new(
                "E0207",
                span,
                format!("{name} expects an address and a value"),
            ));
        };
        let ty = name.strip_prefix("store_").and_then(scalar).unwrap();
        let address = self.expr(address, Some(Type::Ptr))?;
        let value = self.expr(value, Some(ty))?;
        Ok(Stmt::StorePlace(address, value))
    }
}
