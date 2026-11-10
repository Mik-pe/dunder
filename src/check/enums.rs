use super::*;

impl Checker<'_> {
    pub(super) fn enum_type_expr(&self, e: &ast::Expr) -> Result<Option<Type>, Error> {
        let ty = match &e.kind {
            ast::ExprKind::Name(n) if self.lookup(n).is_none() => {
                self.symbols[self.module].records.get(n).map(|(ty, _)| *ty)
            }
            ast::ExprKind::Member(base, n) => {
                let ast::ExprKind::Name(alias) = &base.kind else {
                    return Ok(None);
                };
                if self.lookup(alias).is_some() {
                    return Ok(None);
                }
                let Some(owner) = self.symbols[self.module].imports.get(alias) else {
                    return Ok(None);
                };
                if self.symbols[*owner].records.contains_key(n) {
                    Some(lookup_type(
                        self.symbols,
                        self.module,
                        &format!("{alias}.{n}"),
                        e.span,
                    )?)
                } else {
                    None
                }
            }
            _ => None,
        };
        Ok(ty.filter(|ty| self.types.enumeration(*ty).is_some()))
    }
    pub(super) fn enum_variant(
        &self,
        e: &ast::Expr,
        expected: Option<Type>,
    ) -> Result<Option<(Type, usize)>, Error> {
        let (ty, name) = match &e.kind {
            ast::ExprKind::Variant(n) => {
                let Some(ty) = expected.filter(|ty| self.types.enumeration(*ty).is_some()) else {
                    return Ok(None);
                };
                (ty, n)
            }
            ast::ExprKind::Member(base, n) => {
                let Some(ty) = self.enum_type_expr(base)? else {
                    return Ok(None);
                };
                (ty, n)
            }
            _ => return Ok(None),
        };
        let enumeration = self.types.enumeration(ty).unwrap();
        if enumeration.module != self.module
            && !self.symbols[enumeration.module].records[&enumeration.name].1
        {
            return Err(Error::new(
                "E0403",
                e.span,
                "enum type is private; export it with pub enum",
            ));
        }
        let index = enumeration
            .variants
            .iter()
            .position(|v| v.name == *name)
            .ok_or_else(|| {
                Error::new(
                    "E0271",
                    e.span,
                    format!("{} has no variant '{name}'", enumeration.name),
                )
            })?;
        Ok(Some((ty, index)))
    }
    pub(super) fn enum_construct(
        &mut self,
        e: &ast::Expr,
        args: &[ast::Expr],
        expected: Option<Type>,
    ) -> Result<Expr, Error> {
        let (ty, index) = self.enum_variant(e, expected)?.unwrap();
        let variant = self.types.enumeration(ty).unwrap().variants[index].clone();
        if args.len() != variant.fields.len() {
            return Err(Error::new(
                "E0207",
                e.span,
                format!(
                    "variant {} expects {} values, found {}",
                    variant.name,
                    variant.fields.len(),
                    args.len()
                ),
            ));
        }
        let mut fields = Vec::new();
        for (arg, field) in args.iter().zip(variant.fields) {
            fields.push((field.offset, self.expr(arg, Some(field.ty))?));
        }
        Ok(Expr {
            ty,
            kind: ExprKind::Enum {
                tag: variant.tag,
                fields,
                storage: self.storage(ty),
            },
        })
    }
    pub(super) fn check_match(
        &mut self,
        source: &ast::Expr,
        arms: &[ast::MatchArm],
        span: Span,
    ) -> Result<(Stmt, bool), Error> {
        let source = self.expr(source, None)?;
        let enumeration = self
            .types
            .enumeration(source.ty)
            .cloned()
            .ok_or_else(|| Error::new("E0272", span, "match requires an enum value"))?;
        let mut seen = std::collections::HashSet::new();
        let mut fallback = false;
        let mut checked = Vec::new();
        let mut returns = true;
        for arm in arms {
            if fallback {
                return Err(Error::new(
                    "E0272",
                    arm.span,
                    "a wildcard must be the final match arm",
                ));
            }
            let (pattern, args) = match &arm.pattern.kind {
                ast::ExprKind::Call(callee, args) => (callee.as_ref(), args.as_slice()),
                _ => (&arm.pattern, &[][..]),
            };
            let wildcard = matches!(&pattern.kind,ast::ExprKind::Name(n) if n=="_");
            let mut bindings = Vec::new();
            self.scopes.push(HashMap::new());
            let tag = if wildcard {
                if !args.is_empty() {
                    return Err(Error::new("E0272", arm.span, "a wildcard has no payload"));
                }
                fallback = true;
                None
            } else {
                let (ty, index) = self
                    .enum_variant(pattern, Some(source.ty))?
                    .ok_or_else(|| Error::new("E0272", arm.span, "use an enum variant pattern"))?;
                if ty != source.ty {
                    return Err(mismatch(self.types, arm.span, source.ty, ty));
                }
                if !seen.insert(index) {
                    return Err(Error::new("E0272", arm.span, "duplicate match variant"));
                }
                let variant = &enumeration.variants[index];
                if args.len() != variant.fields.len() {
                    return Err(Error::new(
                        "E0272",
                        arm.span,
                        "pattern bindings must match the variant payload",
                    ));
                }
                for (arg, field) in args.iter().zip(&variant.fields) {
                    let ast::ExprKind::Name(name) = &arg.kind else {
                        return Err(Error::new(
                            "E0272",
                            arg.span,
                            "a payload binding must be a name or '_'",
                        ));
                    };
                    if name == "_" {
                        continue;
                    }
                    let slot = self.bind(name, field.ty, false, arg.span)?;
                    let b = self.lookup(name).unwrap();
                    bindings.push(EnumBinding {
                        ty: field.ty,
                        offset: field.offset,
                        slot,
                        storage: b.storage,
                    });
                }
                Some(variant.tag)
            };
            let (body, ends) = self.block(&arm.body)?;
            self.scopes.pop();
            returns &= ends;
            checked.push(EnumArm {
                tag,
                bindings,
                body,
            });
        }
        if !fallback && seen.len() != enumeration.variants.len() {
            let missing = enumeration
                .variants
                .iter()
                .enumerate()
                .filter(|(i, _)| !seen.contains(i))
                .map(|(_, v)| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::new(
                "E0272",
                span,
                format!("non-exhaustive match; missing {missing}"),
            ));
        }
        let slot = self.slot();
        let storage = self.storage(source.ty);
        Ok((
            Stmt::Match {
                source,
                slot,
                storage,
                arms: checked,
            },
            returns,
        ))
    }
}

pub(super) fn define_enums(
    modules: &[Module],
    symbols: &[Symbols],
    types: &mut Types,
) -> Result<(), Error> {
    let mut definitions = Vec::new();
    for (module, input) in modules.iter().enumerate() {
        for e in &input.enums {
            let ty = symbols[module].records[&e.name].0;
            let mut variants = Vec::new();
            let mut names = std::collections::HashSet::new();
            let mut tags = std::collections::HashSet::new();
            let mut next = 0i128;
            if e.variants.is_empty() {
                return Err(
                    Error::new("E0270", e.span, "an enum needs at least one variant")
                        .in_file(module),
                );
            }
            for variant in &e.variants {
                if reserved(&variant.name) || !names.insert(variant.name.clone()) {
                    return Err(Error::new(
                        "E0270",
                        variant.span,
                        "duplicate or reserved variant name",
                    )
                    .in_file(module));
                }
                let tag = i32::try_from(variant.tag.unwrap_or(next)).map_err(|_| {
                    Error::new("E0270", variant.span, "enum tags must fit i32").in_file(module)
                })?;
                if !tags.insert(tag) {
                    return Err(
                        Error::new("E0270", variant.span, "duplicate enum tag value")
                            .in_file(module),
                    );
                }
                next = i128::from(tag) + 1;
                let mut fields = Vec::new();
                for field in &variant.fields {
                    let ty = resolve_unvalidated(types, symbols, *field, variant.span)
                        .map_err(|e| e.in_file(module))?;
                    fields.push(ast::RecordField {
                        name: String::new(),
                        ty,
                        offset: 0,
                    });
                }
                variants.push(ast::Variant {
                    name: variant.name.clone(),
                    tag,
                    fields,
                });
            }
            types.define_enum(ty, variants, 0);
            definitions.push((ty, e.span, module));
        }
    }
    for (ty, span, module) in definitions {
        layout(ty, types, &mut Vec::new(), span).map_err(|e| e.in_file(module))?;
    }
    for (module, input) in modules.iter().enumerate() {
        for record in &input.records {
            layout(
                symbols[module].records[&record.name].0,
                types,
                &mut Vec::new(),
                record.span,
            )
            .map_err(|e| e.in_file(module))?;
        }
    }
    Ok(())
}
fn layout(ty: Type, types: &mut Types, active: &mut Vec<Type>, span: Span) -> Result<usize, Error> {
    if let Some(record) = types.record(ty).cloned() {
        if record.bytes != 0 {
            return Ok(record.bytes);
        }
        if active.contains(&ty) || active.len() >= 128 {
            return Err(Error::new(
                "E0240",
                span,
                "recursive or excessively nested inline structs need indirection",
            ));
        }
        active.push(ty);
        let mut offset = 0;
        let mut alignment = 1;
        let mut fields = record.fields;
        if record.c_layout && fields.is_empty() {
            return Err(Error::new(
                "E0291",
                span,
                "a C-layout struct needs at least one field",
            ));
        }
        for field in &mut fields {
            let size = layout(field.ty, types, active, span)
                .map_err(|e| Error::new("E0240", span, e.message))?;
            let align = if record.c_layout {
                c_alignment(field.ty, types, span)?
            } else if types.storage_bytes(field.ty).is_some() {
                8
            } else {
                size
            };
            alignment = alignment.max(align);
            offset = (offset + align - 1) & !(align - 1);
            field.offset = offset;
            offset += size;
            if offset > 65536 {
                return Err(Error::new(
                    "E0240",
                    span,
                    "a struct may occupy at most 65536 bytes",
                ));
            }
        }
        active.pop();
        let bytes = (offset.max(1) + alignment - 1) & !(alignment - 1);
        types.set_record_alignment(ty, alignment);
        types.define_record(ty, fields, bytes);
        return Ok(bytes);
    }
    if let Some(e) = types.enumeration(ty).cloned() {
        if e.bytes != 0 {
            return Ok(e.bytes);
        }
        if active.contains(&ty) {
            return Err(Error::new(
                "E0270",
                span,
                "recursive inline enums need indirection; use arena handles for now",
            ));
        }
        if active.len() >= 128 {
            return Err(Error::new(
                "E0270",
                span,
                "enum payload nesting is too deep",
            ));
        }
        active.push(ty);
        let mut variants = e.variants;
        let mut bytes = if e.data { 8 } else { 4 };
        for variant in &mut variants {
            let mut offset = 8;
            for field in &mut variant.fields {
                let size = layout(field.ty, types, active, span)?;
                let alignment = if types.storage_bytes(field.ty).is_some() {
                    8
                } else {
                    size
                };
                offset = (offset + alignment - 1) & !(alignment - 1);
                field.offset = offset;
                offset += size;
            }
            bytes = bytes.max(if e.data { (offset + 7) & !7 } else { 4 });
        }
        active.pop();
        if bytes > 65536 {
            return Err(Error::new(
                "E0270",
                span,
                "an enum value may occupy at most 65536 bytes",
            ));
        }
        types.define_enum(ty, variants, bytes);
        return Ok(bytes);
    }
    if let Some(bytes) = types.storage_bytes(ty) {
        return Ok(bytes.max(1));
    }
    match ty {
        _ if ty.unsigned() || matches!(ty, Type::Ptr | Type::ExternFunction(_)) => {
            Ok(types.scalar_size(ty))
        }
        Type::Bool => Ok(1),
        Type::I32 | Type::F32 | Type::Rune => Ok(4),
        Type::I64 | Type::F64 | Type::String | Type::Function(_) => Ok(8),
        _ => Err(Error::new("E0270", span, "unsupported enum payload type")),
    }
}

fn c_alignment(ty: Type, types: &Types, span: Span) -> Result<usize, Error> {
    if ty.numeric() || matches!(ty, Type::Ptr | Type::ExternFunction(_)) {
        return Ok(types.scalar_size(ty));
    }
    if let Some(a) = types.array(ty) {
        if a.len > 0 {
            return c_alignment(a.elem, types, span);
        }
    }
    if let Some(record) = types.record(ty).filter(|r| r.c_layout) {
        return Ok(record.alignment);
    }
    Err(Error::new("E0291", span, "C-layout fields must be numeric, ptr, extern fn, a nonempty fixed array of C scalars, or a C-layout struct"))
}
