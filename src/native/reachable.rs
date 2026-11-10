//! Keep static function values and their transitive callees; no dynamic table.
use crate::check::{Expr, ExprKind, Program, Stmt};
pub(super) fn functions(program: &Program, entry: usize) -> Vec<bool> {
    let mut found = vec![false; program.functions.len()];
    let mut pending = vec![entry];
    while let Some(f) = pending.pop() {
        if found[f] {
            continue;
        }
        found[f] = true;
        if program.functions[f].uses_float_format {
            if let Some(formatter) = program.float_formatter {
                pending.push(formatter);
            }
        }
        statements(&program.functions[f].body, &mut pending);
    }
    found
}
fn expr(e: &Expr, pending: &mut Vec<usize>) {
    use ExprKind::*;
    match &e.kind {
        Function(f) => pending.push(*f),
        Deref(a)
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
        | Cast(a) => expr(a, pending),
        DynamicOp(_, a, b) | IntOp(_, a, b) => {
            expr(a, pending);
            if let Some(b) = b {
                expr(b, pending);
            }
        }
        ElementAddress(a, b) | Range(a, b) | Index(a, b) | Binary(_, a, b) => {
            expr(a, pending);
            expr(b, pending);
        }
        Enum { fields, .. } | Record(fields, _) => {
            for (_, v) in fields {
                expr(v, pending);
            }
        }
        StringOp(_, args)
        | SysCall(_, args, _)
        | MacCall(_, args, _)
        | Vulkan(_, args, _)
        | Array(args, _)
        | Dynamic(args, _) => {
            for v in args {
                expr(v, pending);
            }
        }
        Call(target, args, _) => {
            expr(target, pending);
            for v in args {
                expr(v, pending);
            }
        }
        Number(_) | Float(_) | Bool(_) | String(_) | Local(_) | LocalAddress(_)
        | ReferenceAddress(_) => {}
    }
}
fn statements(body: &[Stmt], pending: &mut Vec<usize>) {
    for s in body {
        match s {
            Stmt::Store(_, a) | Stmt::StoreAggregate(_, _, a) | Stmt::Expr(a) => expr(a, pending),
            Stmt::StorePlace(a, b) => {
                expr(a, pending);
                expr(b, pending);
            }
            Stmt::Return(a) => {
                if let Some(a) = a {
                    expr(a, pending);
                }
            }
            Stmt::Block(body) => statements(body, pending),
            Stmt::If(c, a, b) => {
                expr(c, pending);
                statements(a, pending);
                statements(b, pending);
            }
            Stmt::While(c, b) => {
                expr(c, pending);
                statements(b, pending);
            }
            Stmt::For { source, body, .. } => {
                expr(source, pending);
                statements(body, pending);
            }
            Stmt::Match { source, arms, .. } => {
                expr(source, pending);
                for a in arms {
                    statements(&a.body, pending);
                }
            }
            Stmt::Break | Stmt::Continue | Stmt::DropStorage(_) => {}
        }
    }
}
