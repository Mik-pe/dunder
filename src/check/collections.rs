use super::*;
fn root(e: &ast::Expr) -> Option<&str> {
    match &e.kind {
        ast::ExprKind::Name(n) => Some(n),
        ast::ExprKind::Member(b, _) | ast::ExprKind::Index(b, _) => root(b),
        _ => None,
    }
}
fn indexed(e: &ast::Expr) -> bool {
    match &e.kind {
        ast::ExprKind::Index(_, _) => true,
        ast::ExprKind::Member(b, _) => indexed(b),
        _ => false,
    }
}
fn mutations<'a>(e: &'a ast::Expr, out: &mut std::collections::HashSet<&'a str>) {
    use ast::ExprKind as A;
    match &e.kind {
        A::Borrow(base, mutable) => {
            if *mutable {
                if let Some(n) = root(base) {
                    out.insert(n);
                }
            }
            mutations(base, out);
        }
        A::Call(callee, args) => {
            if let A::Member(base, name) = &callee.kind {
                if matches!(name.as_str(), "push" | "pop" | "reserve" | "clear") {
                    if let Some(n) = root(base) {
                        out.insert(n);
                    }
                }
            }
            mutations(callee, out);
            for a in args {
                mutations(a, out);
            }
        }
        A::Unary(_, a) | A::Member(a, _) | A::Repeat(a, _) => mutations(a, out),
        A::Index(a, b) | A::Binary(_, a, b) | A::Range(a, b) => {
            mutations(a, out);
            mutations(b, out);
        }
        A::Array(values) => {
            for v in values {
                mutations(v, out);
            }
        }
        A::Record(_, fields) => {
            for (_, v, _) in fields {
                mutations(v, out);
            }
        }
        _ => {}
    }
}
impl Checker<'_> {
    pub(super) fn check_index_assignment(
        &self,
        target: &ast::Expr,
        value: &ast::Expr,
    ) -> Result<(), Error> {
        if indexed(target) {
            if let Some(name) = root(target) {
                let mut changed = std::collections::HashSet::new();
                mutations(value, &mut changed);
                if changed.contains(name) {
                    return Err(Error::new(
                        "E0281",
                        value.span,
                        "compute mutations before assigning an array element",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(super) fn integer_method(&self, e: &ast::Expr) -> Option<IntOp> {
        let ast::ExprKind::Member(base, name) = &e.kind else {
            return None;
        };
        if !self.hint(base).is_some_and(Type::integer)
            && !matches!(base.kind, ast::ExprKind::Number(_))
        {
            return None;
        }
        Some(match name.as_str() {
            "bit_and" => IntOp::And,
            "bit_or" => IntOp::Or,
            "bit_xor" => IntOp::Xor,
            "bit_not" => IntOp::Not,
            "shl" => IntOp::Shl,
            "shr" => IntOp::Shr,
            "wrapping_add" => IntOp::WrappingAdd,
            "wrapping_sub" => IntOp::WrappingSub,
            "wrapping_mul" => IntOp::WrappingMul,
            _ => return None,
        })
    }
    pub(super) fn check_integer_method(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Expr],
        span: Span,
    ) -> Result<Expr, Error> {
        let op = self.integer_method(callee).unwrap();
        let ast::ExprKind::Member(base, _) = &callee.kind else {
            unreachable!()
        };
        let base = self.expr(base, None)?;
        let count = usize::from(!matches!(op, IntOp::Not));
        if args.len() != count {
            return Err(Error::new(
                "E0207",
                span,
                format!("integer method expects {count} arguments"),
            ));
        }
        let arg = if let Some(arg) = args.first() {
            Some(Box::new(self.expr(
                arg,
                Some(if matches!(op, IntOp::Shl | IntOp::Shr) {
                    Type::I32
                } else {
                    base.ty
                }),
            )?))
        } else {
            None
        };
        Ok(Expr {
            ty: base.ty,
            kind: ExprKind::IntOp(op, Box::new(base), arg),
        })
    }
    pub(super) fn place(&mut self, e: &ast::Expr, write: bool) -> Result<(Type, Expr), Error> {
        match &e.kind {
            ast::ExprKind::Name(name) => {
                let b = self
                    .lookup(name)
                    .ok_or_else(|| Error::new("E0205", e.span, format!("unknown name '{name}'")))?;
                if let Some((ty, mutable)) = self.types.referenced(b.ty) {
                    if write && !mutable {
                        return Err(Error::new(
                            "E0208",
                            e.span,
                            "cannot modify a shared reference",
                        ));
                    }
                    return Ok((
                        ty,
                        Expr {
                            ty,
                            kind: ExprKind::ReferenceAddress(b.slot),
                        },
                    ));
                }
                if write && !b.mutable {
                    return Err(Error::new(
                        "E0208",
                        e.span,
                        format!("'{name}' is immutable; declare it with var"),
                    ));
                }
                let kind = if b.storage.is_some() {
                    ExprKind::Local(b.slot)
                } else {
                    ExprKind::LocalAddress(b.slot)
                };
                Ok((b.ty, Expr { ty: b.ty, kind }))
            }
            ast::ExprKind::Member(base, name) => {
                let (ty, address) = self.place(base, write)?;
                let field = self
                    .types
                    .record(ty)
                    .and_then(|r| r.fields.iter().find(|f| f.name == *name))
                    .ok_or_else(|| Error::new("E0241", e.span, format!("no field '{name}'")))?;
                Ok((
                    field.ty,
                    Expr {
                        ty: field.ty,
                        kind: ExprKind::Offset(Box::new(address), field.offset),
                    },
                ))
            }
            ast::ExprKind::Index(base, index) => {
                let (ty, address) = self.place(base, write)?;
                let elem = self
                    .types
                    .dynamic_elem(ty)
                    .or_else(|| self.types.array(ty).map(|a| a.elem))
                    .ok_or_else(|| {
                        Error::new("E0221", e.span, "index assignment requires an array")
                    })?;
                let index = self.expr(index, Some(Type::I32))?;
                Ok((
                    elem,
                    Expr {
                        ty: elem,
                        kind: ExprKind::ElementAddress(Box::new(address), Box::new(index)),
                    },
                ))
            }
            _ => Err(Error::new(
                "E0281",
                e.span,
                "borrow or modify a named variable, field or array element",
            )),
        }
    }
    pub(super) fn check_borrow_aliases(&self, args: &[ast::Expr]) -> Result<(), Error> {
        fn root(e: &ast::Expr) -> Option<&str> {
            match &e.kind {
                ast::ExprKind::Name(n) => Some(n),
                ast::ExprKind::Member(b, _) | ast::ExprKind::Index(b, _) => root(b),
                _ => None,
            }
        }
        let mut seen: HashMap<&str, bool> = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            if let ast::ExprKind::Borrow(base, _) = &arg.kind {
                if indexed(base) {
                    if let Some(name) = root(base) {
                        for (j, other) in args.iter().enumerate() {
                            if i != j {
                                let mut changed = std::collections::HashSet::new();
                                mutations(other, &mut changed);
                                if changed.contains(name) {
                                    return Err(Error::new(
                                        "E0281",
                                        other.span,
                                        "compute mutations before borrowing an array element",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        for arg in args {
            if let ast::ExprKind::Borrow(base, mutable) = &arg.kind {
                if let Some(name) = root(base) {
                    if seen.get(name).is_some_and(|other| *other || *mutable) {
                        return Err(Error::new(
                            "E0281",
                            arg.span,
                            "mutable arguments must not alias another borrow in the same call",
                        ));
                    }
                    seen.insert(name, *mutable);
                }
            }
        }
        Ok(())
    }
    pub(super) fn dynamic_method(&self, e: &ast::Expr) -> Option<DynamicOp> {
        let ast::ExprKind::Member(base, name) = &e.kind else {
            return None;
        };
        self.types.dynamic_elem(self.hint(base)?)?;
        Some(match name.as_str() {
            "len" => DynamicOp::Len,
            "cap" => DynamicOp::Capacity,
            "push" => DynamicOp::Push,
            "pop" => DynamicOp::Pop,
            "reserve" => DynamicOp::Reserve,
            "clear" => DynamicOp::Clear,
            _ => return None,
        })
    }
    pub(super) fn check_dynamic_method(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Expr],
        span: Span,
    ) -> Result<Expr, Error> {
        let op = self.dynamic_method(callee).unwrap();
        let ast::ExprKind::Member(base, _) = &callee.kind else {
            unreachable!()
        };
        let count = usize::from(matches!(op, DynamicOp::Push | DynamicOp::Reserve));
        if args.len() != count {
            return Err(Error::new(
                "E0207",
                span,
                format!("method expects {count} arguments"),
            ));
        }
        let mutable = matches!(
            op,
            DynamicOp::Push | DynamicOp::Pop | DynamicOp::Reserve | DynamicOp::Clear
        );
        let (ty, receiver) = if mutable {
            self.place(base, true)?
        } else {
            let value = self.expr(base, None)?;
            (value.ty, value)
        };
        let elem = self.types.dynamic_elem(ty).unwrap();
        let arg = if let Some(arg) = args.first() {
            Some(Box::new(self.expr(
                arg,
                Some(if matches!(op, DynamicOp::Reserve) {
                    Type::I32
                } else {
                    elem
                }),
            )?))
        } else {
            None
        };
        let result = match op {
            DynamicOp::Len | DynamicOp::Capacity => Type::I32,
            DynamicOp::Pop => elem,
            _ => Type::Unit,
        };
        Ok(Expr {
            ty: result,
            kind: ExprKind::DynamicOp(op, Box::new(receiver), arg),
        })
    }
}
