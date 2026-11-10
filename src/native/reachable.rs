//! Keep static function values and their transitive callees; no dynamic table.
use crate::check::{Expr, ExprKind, Program, Stmt};

#[derive(Default)]
pub(super) struct Dependencies {
    pub functions: Vec<usize>,
    pub external_effects: bool,
}

pub(super) fn dependencies(program: &Program, function: usize) -> Dependencies {
    let f = &program.functions[function];
    let mut deps = Dependencies { external_effects: f.external.is_some(), ..Dependencies::default() };
    if f.uses_float_format { deps.functions.extend(program.float_formatter); }
    visit_body(&f.body, false, &mut |e, _| match e.kind {
        ExprKind::Function(index) | ExprKind::Callback(index) => deps.functions.push(index),
        ExprKind::Call(ref t, _, _) if matches!(t.ty, crate::syntax::Type::ExternFunction(_)) => deps.external_effects = true,
        ExprKind::SysCall(..) => deps.external_effects = true,
        _ if matches!(e.ty, crate::syntax::Type::Ptr) => deps.external_effects = true,
        _ => {}
    });
    deps
}
pub(super) fn functions(program: &Program, entry: usize) -> Vec<bool> {
    let mut found = vec![false; program.functions.len()];
    let mut pending = vec![entry];
    while let Some(f) = pending.pop() {
        if found[f] {
            continue;
        }
        found[f] = true;
        pending.extend(dependencies(program, f).functions);
    }
    found
}
pub(super) fn visit_expr(e: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(e);
    use ExprKind::*;
    match &e.kind {
        Function(_) | Callback(_) | StaticBytes { .. } => {}
        ScratchAlloc { count: a, .. } | Deref(a)
        | ManagedString(a, _, _)
        | Offset(a, _)
        | Clone(a, _)
        | EnumFromInt(a)
        | FloatOp(_, a)
        | StringView(a)
        | Field(a, _)
        | Assert(a)
        | Iterator(a)
        | Len(a, _)
        | Repeat(a, _)
        | Print(a)
        | Unary(_, a)
        | Cast(a) => visit_expr(a, visit),
        DynamicOp(_, a, b) | IntOp(_, a, b) => {
            visit_expr(a, visit);
            if let Some(b) = b {
                visit_expr(b, visit);
            }
        }
        ElementAddress(a, b) | Range(a, b) | Index(a, b) | Binary(_, a, b) => {
            visit_expr(a, visit);
            visit_expr(b, visit);
        }
        Enum { fields, .. } | Record(fields, _) => {
            for (_, v) in fields {
                visit_expr(v, visit);
            }
        }
        StringOp(_, args)
        | SysCall(_, args, _)
        | Array(args, _)
        | Dynamic(args, _) => {
            for v in args {
                visit_expr(v, visit);
            }
        }
        Call(target, args, _) => {
            visit_expr(target, visit);
            for v in args {
                visit_expr(v, visit);
            }
        }
        Number(_) | Float(_) | Bool(_) | String(_) | Local(_) | LocalAddress(_)
        | ReferenceAddress(_) => {}
    }
}
/// Visit every expression, including addresses outside loops which may alias a
/// local that is read inside a loop.
pub(super) fn visit_body(body: &[Stmt], in_loop: bool, visit: &mut impl FnMut(&Expr, bool)) {
    for s in body {
        let mut expr = |e: &Expr, repeated: bool| visit_expr(e, &mut |e| visit(e, repeated));
        match s {
            Stmt::Store(_, a) | Stmt::StoreAggregate(_, _, a) | Stmt::Expr(a) => expr(a, in_loop),
            Stmt::StorePlace(a, b) => {
                expr(a, in_loop);
                expr(b, in_loop);
            }
            Stmt::Return(Some(a)) => expr(a, in_loop),
            Stmt::Block(body) => visit_body(body, in_loop, visit),
            Stmt::If(c, a, b) => {
                expr(c, in_loop);
                visit_body(a, in_loop, visit);
                visit_body(b, in_loop, visit);
            }
            Stmt::While(c, b) => {
                expr(c, true);
                visit_body(b, true, visit);
            }
            Stmt::For { source, body, .. } => {
                expr(source, in_loop);
                visit_body(body, true, visit);
            }
            Stmt::Match { source, arms, .. } => {
                expr(source, in_loop);
                for a in arms {
                    visit_body(&a.body, in_loop, visit);
                }
            }
            Stmt::Return(None) | Stmt::Break | Stmt::Continue | Stmt::DropStorage(_) => {}
        }
    }
}
