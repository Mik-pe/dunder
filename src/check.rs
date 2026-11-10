use crate::{
    syntax::{self as ast, Type, Types},
    Error, Span,
};
use std::collections::HashMap;

pub struct Program {
    pub types: Types,
    pub functions: Vec<Function>,
    pub main: usize,
}
pub struct Function {
    pub name: String,
    pub params: Vec<Type>,
    pub slots: usize,
    pub body: Vec<Stmt>,
}
pub enum Stmt {
    Store(usize, Expr),
    Return(Option<Expr>),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Block(Vec<Stmt>),
    Break,
    Continue,
}
pub struct Expr {
    pub ty: Type,
    pub kind: ExprKind,
}
pub enum ExprKind {
    Number(i64),
    Bool(bool),
    Local(usize),
    Function(usize),
    Call(Box<Expr>, Vec<Expr>),
    Print(Box<Expr>),
    Cast(Box<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    slot: usize,
    mutable: bool,
}
struct Checker<'a> {
    signatures: &'a HashMap<String, (usize, Type)>,
    types: &'a Types,
    scopes: Vec<HashMap<String, Binding>>,
    slots: usize,
    ret: Type,
    loops: usize,
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "fn" | "var"
            | "return"
            | "if"
            | "else"
            | "while"
            | "true"
            | "false"
            | "break"
            | "continue"
            | "i32"
            | "i64"
            | "bool"
            | "print"
    )
}
fn mismatch(types: &Types, span: Span, expected: Type, actual: Type) -> Error {
    Error::new(
        "E0201",
        span,
        format!(
            "expected {}, found {}",
            types.name(expected),
            types.name(actual)
        ),
    )
}

impl Checker<'_> {
    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned()
    }
    fn bind(&mut self, name: &str, ty: Type, mutable: bool, span: Span) -> Result<usize, Error> {
        if reserved(name) {
            return Err(Error::new("E0200", span, format!("'{name}' is reserved")));
        }
        if ty == Type::Unit {
            return Err(Error::new(
                "E0202",
                span,
                "a variable or parameter must have a value type",
            ));
        }
        if self.scopes.last().unwrap().contains_key(name) {
            return Err(Error::new(
                "E0203",
                span,
                format!("'{name}' is already declared in this scope"),
            ));
        }
        let slot = self.slots;
        self.slots += 1;
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.into(), Binding { ty, slot, mutable });
        Ok(slot)
    }
    fn hint(&self, expr: &ast::Expr) -> Option<Type> {
        match &expr.kind {
            ast::ExprKind::Name(n) => self
                .lookup(n)
                .map(|b| b.ty)
                .or_else(|| self.signatures.get(n).map(|(_, ty)| *ty)),
            ast::ExprKind::Call(callee, _) => {
                if let ast::ExprKind::Name(n) = &callee.kind {
                    if n == "i32" {
                        return Some(Type::I32);
                    }
                    if n == "i64" {
                        return Some(Type::I64);
                    }
                }
                self.hint(callee)
                    .and_then(|ty| self.types.signature(ty).map(|f| f.ret))
            }
            ast::ExprKind::Unary("-", e) => self.hint(e),
            ast::ExprKind::Binary(op, a, b) if matches!(*op, "+" | "-" | "*" | "/" | "%") => {
                self.hint(a).or_else(|| self.hint(b))
            }
            _ => None,
        }
    }
    fn number(&self, value: i128, ty: Type, span: Span) -> Result<Expr, Error> {
        let valid = match ty {
            Type::I32 => i32::try_from(value).is_ok(),
            Type::I64 => i64::try_from(value).is_ok(),
            _ => false,
        };
        if !valid {
            return Err(Error::new("E0204", span, format!("integer literal {value} does not fit {}; use an explicit integer type if needed", ty.name())));
        }
        Ok(Expr {
            ty,
            kind: ExprKind::Number(value as i64),
        })
    }
    fn expr(&mut self, e: &ast::Expr, expected: Option<Type>) -> Result<Expr, Error> {
        use ast::ExprKind as A;
        let typed = match &e.kind {
            A::Number(n) => self.number(
                *n,
                expected.filter(|t| t.integer()).unwrap_or(Type::I32),
                e.span,
            )?,
            A::Bool(b) => Expr {
                ty: Type::Bool,
                kind: ExprKind::Bool(*b),
            },
            A::Name(n) => {
                if let Some(b) = self.lookup(n) {
                    Expr {
                        ty: b.ty,
                        kind: ExprKind::Local(b.slot),
                    }
                } else if let Some((index, ty)) = self.signatures.get(n) {
                    Expr {
                        ty: *ty,
                        kind: ExprKind::Function(*index),
                    }
                } else {
                    return Err(Error::new("E0205", e.span, format!("unknown name '{n}'")));
                }
            }
            A::Unary("-", inner) if matches!(inner.kind, A::Number(_)) => {
                let A::Number(n) = inner.kind else {
                    unreachable!()
                };
                self.number(
                    -n,
                    expected.filter(|t| t.integer()).unwrap_or(Type::I32),
                    e.span,
                )?
            }
            A::Unary(op, inner) => {
                let x = self.expr(
                    inner,
                    if *op == "!" {
                        Some(Type::Bool)
                    } else {
                        expected
                    },
                )?;
                if *op == "-" && !x.ty.integer() {
                    return Err(Error::new("E0206", e.span, "unary '-' requires an integer"));
                }
                Expr {
                    ty: x.ty,
                    kind: ExprKind::Unary(op, Box::new(x)),
                }
            }
            A::Binary(op, left, right) => {
                if matches!(*op, "&&" | "||") {
                    let a = self.expr(left, Some(Type::Bool))?;
                    let b = self.expr(right, Some(Type::Bool))?;
                    Expr {
                        ty: Type::Bool,
                        kind: ExprKind::Binary(op, Box::new(a), Box::new(b)),
                    }
                } else {
                    let arithmetic = matches!(*op, "+" | "-" | "*" | "/" | "%");
                    let hint = if arithmetic {
                        expected
                            .filter(|t| t.integer())
                            .or_else(|| self.hint(left))
                            .or_else(|| self.hint(right))
                    } else {
                        self.hint(left).or_else(|| self.hint(right))
                    };
                    let a = self.expr(left, hint)?;
                    let b = self.expr(right, Some(a.ty))?;
                    if !a.ty.integer() && !(matches!(*op, "==" | "!=") && a.ty == Type::Bool) {
                        return Err(Error::new(
                            "E0206",
                            e.span,
                            format!("operator '{op}' cannot be used with {}", a.ty.name()),
                        ));
                    }
                    Expr {
                        ty: if arithmetic { a.ty } else { Type::Bool },
                        kind: ExprKind::Binary(op, Box::new(a), Box::new(b)),
                    }
                }
            }
            A::Call(callee, args) if matches!(&callee.kind, A::Name(n) if n == "print") => {
                if args.len() != 1 {
                    return Err(Error::new("E0207", e.span, "print expects one argument"));
                }
                let value = self.expr(&args[0], None)?;
                if !matches!(value.ty, Type::I32 | Type::I64 | Type::Bool) {
                    return Err(Error::new(
                        "E0206",
                        e.span,
                        "print requires i32, i64 or bool",
                    ));
                }
                Expr {
                    ty: Type::Unit,
                    kind: ExprKind::Print(Box::new(value)),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind, A::Name(n) if n == "i32" || n == "i64") =>
            {
                if args.len() != 1 {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "an integer conversion expects one argument",
                    ));
                }
                let ty = if matches!(&callee.kind, A::Name(n) if n == "i32") {
                    Type::I32
                } else {
                    Type::I64
                };
                let inner = self.expr(&args[0], self.hint(&args[0]).or(Some(ty)))?;
                if !inner.ty.integer() {
                    return Err(Error::new(
                        "E0206",
                        e.span,
                        "integer conversion requires an integer; bool is a separate type",
                    ));
                }
                Expr {
                    ty,
                    kind: ExprKind::Cast(Box::new(inner)),
                }
            }
            A::Call(callee, args) => {
                let target = self.expr(callee, None)?;
                let sig = self.types.signature(target.ty).ok_or_else(|| {
                    Error::new(
                        "E0206",
                        callee.span,
                        format!(
                            "{} is not callable; expected a function",
                            self.types.name(target.ty)
                        ),
                    )
                })?;
                if args.len() != sig.params.len() {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        format!(
                            "function expects {} arguments, found {}",
                            sig.params.len(),
                            args.len()
                        ),
                    ));
                }
                let ret = sig.ret;
                let types = sig.params.clone();
                let values = args
                    .iter()
                    .zip(types)
                    .map(|(a, t)| self.expr(a, Some(t)))
                    .collect::<Result<Vec<_>, _>>()?;
                Expr {
                    ty: ret,
                    kind: ExprKind::Call(Box::new(target), values),
                }
            }
        };
        if let Some(t) = expected {
            if t != typed.ty {
                return Err(mismatch(self.types, e.span, t, typed.ty));
            }
        }
        Ok(typed)
    }
    fn block(&mut self, stmts: &[ast::Stmt]) -> Result<(Vec<Stmt>, bool), Error> {
        self.scopes.push(HashMap::new());
        let result = self.statements(stmts);
        self.scopes.pop();
        result
    }
    fn statements(&mut self, stmts: &[ast::Stmt]) -> Result<(Vec<Stmt>, bool), Error> {
        let mut result = Vec::new();
        let mut returns = false;
        for s in stmts {
            let (stmt, ends) = match &s.kind {
                ast::StmtKind::Decl {
                    name,
                    ty,
                    mutable,
                    init,
                } => {
                    let x = self.expr(init, *ty)?;
                    let slot = self.bind(name, x.ty, *mutable, s.span)?;
                    (Stmt::Store(slot, x), false)
                }
                ast::StmtKind::Assign(n, value) => {
                    let b = self.lookup(n).ok_or_else(|| {
                        Error::new(
                            "E0205",
                            s.span,
                            format!("unknown variable '{n}'; use ':=' to declare it"),
                        )
                    })?;
                    if !b.mutable {
                        return Err(Error::new("E0208", s.span, format!("'{n}' is immutable; declare it with 'var {n} := ...' to allow assignment")));
                    }
                    (Stmt::Store(b.slot, self.expr(value, Some(b.ty))?), false)
                }
                ast::StmtKind::Return(value) => {
                    let v = value
                        .as_ref()
                        .map(|v| self.expr(v, Some(self.ret)))
                        .transpose()?;
                    if v.is_none() && self.ret != Type::Unit {
                        return Err(mismatch(self.types, s.span, self.ret, Type::Unit));
                    }
                    (Stmt::Return(v), true)
                }
                ast::StmtKind::Expr(e) => {
                    let x = self.expr(e, None)?;
                    if x.ty != Type::Unit {
                        return Err(Error::new(
                            "E0209",
                            s.span,
                            "unused value; assign with ':=' or return it",
                        ));
                    }
                    (Stmt::Expr(x), false)
                }
                ast::StmtKind::If(c, a, b) => {
                    let cond = self.expr(c, Some(Type::Bool))?;
                    let (a, ar) = self.block(a)?;
                    let (b, br) = self.block(b)?;
                    (Stmt::If(cond, a, b), ar && br)
                }
                ast::StmtKind::While(c, body) => {
                    let cond = self.expr(c, Some(Type::Bool))?;
                    self.loops += 1;
                    let (body, _) = self.block(body)?;
                    self.loops -= 1;
                    (Stmt::While(cond, body), false)
                }
                ast::StmtKind::Block(b) => {
                    let (b, r) = self.block(b)?;
                    (Stmt::Block(b), r)
                }
                ast::StmtKind::Break | ast::StmtKind::Continue => {
                    if self.loops == 0 {
                        return Err(Error::new(
                            "E0210",
                            s.span,
                            "break/continue requires a while loop",
                        ));
                    }
                    (
                        if matches!(s.kind, ast::StmtKind::Break) {
                            Stmt::Break
                        } else {
                            Stmt::Continue
                        },
                        false,
                    )
                }
            };
            returns |= ends;
            result.push(stmt);
        }
        Ok((result, returns))
    }
}

pub fn check(program: ast::Program) -> Result<Program, Error> {
    let ast::Program {
        functions,
        mut types,
    } = program;
    let mut signatures = HashMap::new();
    for (i, f) in functions.iter().enumerate() {
        if reserved(&f.name) {
            return Err(Error::new(
                "E0200",
                f.span,
                format!("'{}' is reserved", f.name),
            ));
        }
        if signatures
            .insert(
                f.name.clone(),
                (
                    i,
                    types.function(f.params.iter().map(|p| p.ty).collect(), f.ret),
                ),
            )
            .is_some()
        {
            return Err(Error::new(
                "E0203",
                f.span,
                format!("duplicate function '{}'", f.name),
            ));
        }
    }
    let span = Span { start: 0, end: 0 };
    let (main, main_type) = signatures.get("main").ok_or_else(|| {
        Error::new(
            "E0212",
            span,
            "program needs 'fn main()' or 'fn main() -> i32'",
        )
    })?;
    let sig = types.signature(*main_type).unwrap();
    if !sig.params.is_empty() || !matches!(sig.ret, Type::I32 | Type::Unit) {
        return Err(Error::new(
            "E0212",
            functions[*main].span,
            "main must take no arguments and return i32 or ()",
        ));
    }
    let main = *main;
    let mut checked = Vec::new();
    for f in functions {
        let mut c = Checker {
            signatures: &signatures,
            types: &types,
            scopes: vec![HashMap::new()],
            slots: 0,
            ret: f.ret,
            loops: 0,
        };
        for p in &f.params {
            c.bind(&p.name, p.ty, false, p.span)?;
        }
        let (body, returns) = c.statements(&f.body)?;
        if f.ret != Type::Unit && !returns {
            return Err(Error::new(
                "E0213",
                f.span,
                format!(
                    "function '{}' must return {} on every path",
                    f.name,
                    f.ret.name()
                ),
            ));
        }
        checked.push(Function {
            name: f.name,
            params: f.params.iter().map(|p| p.ty).collect(),
            slots: c.slots,
            body,
        });
    }
    Ok(Program {
        types,
        functions: checked,
        main,
    })
}
