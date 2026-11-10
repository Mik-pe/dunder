//! A region owns one byte buffer. Its views are lexical bindings, never
//! independently owned dynamic arrays. Element reads produce ordinary scalars.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct Arena {
    cursor: usize,
    limit: usize,
}

#[derive(Default)]
pub(super) struct Regions {
    areas: HashMap<usize, Arena>,
    slices: HashSet<usize>,
}

fn local(slot: usize, ty: Type) -> Expr {
    Expr {
        ty,
        kind: ExprKind::Local(slot),
    }
}

fn integer(n: i64) -> Expr {
    Expr {
        ty: Type::I32,
        kind: ExprKind::Number(n),
    }
}

fn scratch_element(types: &Types, ty: Type) -> bool {
    if let Some(r) = types.record(ty) {
        return r.fields.iter().all(|f| scratch_element(types, f.ty));
    }
    if let Some(a) = types.array(ty) {
        return scratch_element(types, a.elem);
    }
    ty.numeric() || matches!(ty, Type::Bool | Type::Rune)
}

impl Checker<'_> {
    pub(super) fn check_region(
        &mut self,
        name: &str,
        capacity: Option<&ast::Expr>,
        statements: &[ast::Stmt],
        span: Span,
    ) -> Result<(Stmt, bool), Error> {
        let capacity = match capacity {
            Some(value) => self.expr(value, Some(Type::I32))?,
            None => integer(64 * 1024),
        };
        let owner_start = self.owned_storage.len();
        let first_slot = self.slots;
        self.scopes.push(HashMap::new());
        // A bool occupies one byte. This private descriptor is never indexed as bool.
        let backing = self.types.dynamic(Type::Bool, span)?;
        let area = self.bind(name, backing, false, span)?;
        let storage = self.lookup(name).unwrap().storage.unwrap();
        let cursor = self.slot();
        let limit = self.slot();
        self.regions.areas.insert(area, Arena { cursor, limit });
        let mut body = vec![
            Stmt::Store(limit, capacity),
            Stmt::Store(cursor, integer(0)),
            Stmt::Store(
                area,
                Expr {
                    ty: backing,
                    kind: ExprKind::Dynamic(vec![], storage),
                },
            ),
            Stmt::Expr(Expr {
                ty: Type::Unit,
                kind: ExprKind::DynamicOp(
                    DynamicOp::Reserve,
                    Box::new(local(area, backing)),
                    Some(Box::new(local(limit, Type::I32))),
                ),
            }),
        ];
        let (user, returns) = self.statements(statements)?;
        body.extend(user);
        body.push(Stmt::DropStorage(
            self.owned_storage[owner_start..].to_vec(),
        ));
        self.scopes.pop();
        self.regions.areas.remove(&area);
        self.regions.slices.retain(|&slot| slot < first_slot);
        Ok((Stmt::Block(body), returns))
    }

    pub(super) fn scratch_declaration(
        &mut self,
        name: &str,
        ty: Option<Type>,
        mutable: bool,
        init: &ast::Expr,
        span: Span,
    ) -> Result<Option<Stmt>, Error> {
        if self.regions.areas.is_empty() {
            return Ok(None);
        }
        let ast::ExprKind::Call(callee, args) = &init.kind else {
            return Ok(None);
        };
        let ast::ExprKind::Member(base, method) = &callee.kind else {
            return Ok(None);
        };
        let ast::ExprKind::Name(region_name) = &base.kind else {
            return Ok(None);
        };
        if method != "alloc" {
            return Ok(None);
        }
        let Some(area) = self.lookup(region_name).map(|b| b.slot) else {
            return Ok(None);
        };
        let Some(arena) = self.regions.areas.get(&area).copied() else {
            return Ok(None);
        };
        let [count] = args.as_slice() else {
            return Err(Error::new(
                "E0207",
                init.span,
                "region.alloc expects an element count",
            ));
        };
        let elem = ty
            .and_then(|t| self.types.dynamic_elem(t))
            .map(|t| resolve_type(self.types, self.symbols, t, span))
            .transpose()?
            .filter(|t| scratch_element(self.types, *t))
            .ok_or_else(|| {
                Error::new("E0282", span,
                "declare a scratch slice as []T with a numeric, bool, rune or plain record element")
            })?;
        let ty = self.types.intern_dynamic(elem);
        let count = Box::new(self.expr(count, Some(Type::I32))?);
        let slot = self.bind_local(name, ty, mutable, span, false)?;
        // A view has the native dynamic-array descriptor layout, but is never
        // entered in owned_storage. The region releases its buffer once.
        self.slots += self.types.storage_bytes(ty).unwrap().div_ceil(8);
        let storage = self.slots - 1;
        self.regions.slices.insert(slot);
        Ok(Some(Stmt::Store(
            slot,
            Expr {
                ty,
                kind: ExprKind::ScratchAlloc {
                    area,
                    cursor: arena.cursor,
                    limit: arena.limit,
                    count,
                    storage,
                },
            },
        )))
    }

    pub(super) fn scratch_slice(&self, expr: &ast::Expr) -> Option<Binding> {
        if self.regions.slices.is_empty() {
            return None;
        }
        let ast::ExprKind::Name(name) = &expr.kind else {
            return None;
        };
        self.lookup(name)
            .filter(|b| self.regions.slices.contains(&b.slot))
    }

    pub(super) fn index_receiver(&mut self, expr: &ast::Expr) -> Result<Expr, Error> {
        if let Some(b) = self.scratch_slice(expr) {
            return Ok(local(b.slot, b.ty));
        }
        self.expr(expr, None)
    }

    pub(super) fn check_scratch_value(&self, binding: &Binding, span: Span) -> Result<(), Error> {
        if self.regions.slices.contains(&binding.slot) {
            return Err(Error::new(
                "E0282",
                span,
                "scratch slices stay in their region; read or write an element or take the length",
            ));
        }
        if self.regions.areas.contains_key(&binding.slot) {
            return Err(Error::new(
                "E0282",
                span,
                "a region is only usable as the receiver of alloc in a typed slice declaration",
            ));
        }
        Ok(())
    }

    pub(super) fn check_scratch_borrow(&self, mut expr: &ast::Expr) -> Result<(), Error> {
        while let ast::ExprKind::Index(base, _) | ast::ExprKind::Member(base, _) = &expr.kind {
            expr = base;
        }
        if self.scratch_slice(expr).is_some() {
            return Err(Error::new(
                "E0282",
                expr.span,
                "scratch storage cannot be borrowed outside its binding",
            ));
        }
        Ok(())
    }
}
