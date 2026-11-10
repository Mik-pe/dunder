use crate::{
    syntax::{self as ast, Type, Types},
    Error, Span,
};
use std::collections::HashMap;

pub struct Program {
    pub types: Types,
    pub functions: Vec<Function>,
    pub main: Option<usize>,
    pub tests: Vec<TestCase>,
}
pub struct TestCase {
    pub name: String,
    pub function: usize,
    pub file: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Program,
    Test,
}
pub struct Module {
    pub functions: Vec<ast::Function>,
    pub imports: Vec<Import>,
    pub records: Vec<ast::Record>,
}
pub struct Import {
    pub name: String,
    pub target: usize,
    pub span: Span,
}
struct Global {
    index: usize,
    ty: Type,
    public: bool,
}
#[derive(Default)]
struct Symbols {
    functions: HashMap<String, Global>,
    imports: HashMap<String, usize>,
    records: HashMap<String, (Type, bool)>,
}
pub struct Function {
    pub name: String,
    pub params: Vec<Type>,
    pub slots: usize,
    pub body: Vec<Stmt>,
    pub param_slots: Vec<usize>,
    pub param_storage: Vec<Option<usize>>,
}
pub enum Stmt {
    Store(usize, Expr),
    StoreAggregate(usize, usize, Expr),
    StoreIndex(Expr, Expr, Expr),
    StoreField(Expr, usize, Expr),
    Return(Option<Expr>),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For {
        source: Expr,
        source_slot: usize,
        cursor: usize,
        end: usize,
        binding: usize,
        storage: Option<usize>,
        body: Vec<Stmt>,
    },
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
    Array(Vec<Expr>, usize),
    Repeat(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>),
    Len(Box<Expr>, usize),
    Iterator(Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Assert(Box<Expr>),
    Record(Vec<(usize, Expr)>, usize),
    Field(Box<Expr>, usize),
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    slot: usize,
    mutable: bool,
    storage: Option<usize>,
}
struct Checker<'a> {
    module: usize,
    symbols: &'a [Symbols],
    types: &'a mut Types,
    scopes: Vec<HashMap<String, Binding>>,
    slots: usize,
    ret: Type,
    loops: usize,
    testing: bool,
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
            | "import"
            | "pub"
            | "as"
            | "for"
            | "in"
            | "len"
            | "range"
            | "assert"
            | "struct"
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
    fn slot(&mut self) -> usize {
        let slot = self.slots;
        self.slots += 1;
        slot
    }
    fn storage(&mut self, ty: Type) -> Option<usize> {
        self.types.storage_bytes(ty).map(|bytes| {
            let words = bytes.div_ceil(8).max(1);
            self.slots += words;
            self.slots - 1
        })
    }
    fn constructor_type(&self, expr: &ast::Expr) -> Result<Type, Error> {
        let name = match &expr.kind {
            ast::ExprKind::Name(n) => n.clone(),
            ast::ExprKind::Member(base, n) => {
                let ast::ExprKind::Name(alias) = &base.kind else {
                    return Err(Error::new(
                        "E0240",
                        expr.span,
                        "name a struct type in a constructor",
                    ));
                };
                format!("{alias}.{n}")
            }
            _ => {
                return Err(Error::new(
                    "E0240",
                    expr.span,
                    "name a struct type in a constructor",
                ))
            }
        };
        lookup_type(self.symbols, self.module, &name, expr.span)
    }
    fn store_value(&self, binding: &Binding, value: Expr) -> Stmt {
        if let Some(storage) = binding.storage {
            Stmt::StoreAggregate(binding.slot, storage, value)
        } else {
            Stmt::Store(binding.slot, value)
        }
    }
    fn global(&self, name: &str) -> Option<(usize, Type)> {
        self.symbols[self.module]
            .functions
            .get(name)
            .map(|g| (g.index, g.ty))
    }
    fn module_alias(&self, base: &ast::Expr) -> bool {
        matches!(&base.kind,ast::ExprKind::Name(n) if self.lookup(n).is_none()&&self.symbols[self.module].imports.contains_key(n))
    }
    fn member(&self, base: &ast::Expr, name: &str, span: Span) -> Result<(usize, Type), Error> {
        let ast::ExprKind::Name(alias) = &base.kind else {
            return Err(Error::new(
                "E0405",
                span,
                "module access must use an imported alias, such as math.add",
            ));
        };
        if self.lookup(alias).is_some() {
            return Err(Error::new(
                "E0405",
                span,
                format!("'{alias}' is a local value, not a module"),
            ));
        }
        let target = self.symbols[self.module]
            .imports
            .get(alias)
            .ok_or_else(|| {
                Error::new(
                    "E0404",
                    base.span,
                    format!("unknown module alias '{alias}'"),
                )
            })?;
        let function = self.symbols[*target].functions.get(name).ok_or_else(|| {
            Error::new(
                "E0404",
                span,
                format!("module '{alias}' has no function '{name}'"),
            )
        })?;
        if !function.public {
            return Err(Error::new(
                "E0403",
                span,
                format!("'{alias}.{name}' is private; export it with 'pub fn'"),
            ));
        }
        Ok((function.index, function.ty))
    }
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
        let slot = self.slot();
        let storage = self.storage(ty);
        self.scopes.last_mut().unwrap().insert(
            name.into(),
            Binding {
                ty,
                slot,
                mutable,
                storage,
            },
        );
        Ok(slot)
    }
    fn hint(&self, expr: &ast::Expr) -> Option<Type> {
        match &expr.kind {
            ast::ExprKind::TestTarget(n) => self.global(n).map(|(_, ty)| ty),
            ast::ExprKind::Name(n) => self
                .lookup(n)
                .map(|b| b.ty)
                .or_else(|| self.global(n).map(|(_, ty)| ty)),
            ast::ExprKind::Member(base, name) => self
                .hint(base)
                .and_then(|ty| self.types.record(ty))
                .and_then(|r| r.fields.iter().find(|f| f.name == *name))
                .map(|f| f.ty)
                .or_else(|| self.member(base, name, expr.span).ok().map(|(_, ty)| ty)),
            ast::ExprKind::Record(name, _) => self.constructor_type(name).ok(),
            ast::ExprKind::Call(callee, _) => {
                if let ast::ExprKind::Name(n) = &callee.kind {
                    if n == "i32" {
                        return Some(Type::I32);
                    }
                    if n == "i64" {
                        return Some(Type::I64);
                    }
                    if n == "len" {
                        return Some(Type::I32);
                    }
                }
                if let ast::ExprKind::Member(base, name) = &callee.kind {
                    if let Some(ty) = self.hint(base) {
                        if self.types.array(ty).is_some() {
                            if name == "len" {
                                return Some(Type::I32);
                            }
                            if name == "iter" {
                                if let Type::Array(i) | Type::Iterator(i) = ty {
                                    return Some(Type::Iterator(i));
                                }
                            }
                        }
                    }
                }
                self.hint(callee)
                    .and_then(|ty| self.types.signature(ty).map(|f| f.ret))
            }
            ast::ExprKind::Unary("-", e) => self.hint(e),
            ast::ExprKind::Index(base, _) => self
                .hint(base)
                .and_then(|t| self.types.array(t))
                .map(|a| a.elem),
            ast::ExprKind::Range(_, _) => Some(Type::Range),
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
            A::Record(name, fields) => {
                let ty = self.constructor_type(name)?;
                let record = self.types.record(ty).unwrap().clone();
                let mut seen = std::collections::HashSet::new();
                let mut values = Vec::new();
                for (name, value, span) in fields {
                    let field =
                        record
                            .fields
                            .iter()
                            .find(|f| f.name == *name)
                            .ok_or_else(|| {
                                Error::new(
                                    "E0241",
                                    *span,
                                    format!("{} has no field '{name}'", record.name),
                                )
                            })?;
                    if !seen.insert(name.clone()) {
                        return Err(Error::new(
                            "E0241",
                            *span,
                            format!("duplicate field '{name}'"),
                        ));
                    }
                    values.push((field.offset, self.expr(value, Some(field.ty))?));
                }
                let missing: Vec<_> = record
                    .fields
                    .iter()
                    .filter(|f| !seen.contains(&f.name))
                    .map(|f| f.name.as_str())
                    .collect();
                if !missing.is_empty() {
                    return Err(Error::new(
                        "E0241",
                        e.span,
                        format!("missing fields: {}", missing.join(", ")),
                    ));
                }
                let storage = self.storage(ty).unwrap();
                Expr {
                    ty,
                    kind: ExprKind::Record(values, storage),
                }
            }
            A::Range(a, b) => {
                let a = self.expr(a, Some(Type::I32))?;
                let b = self.expr(b, Some(Type::I32))?;
                Expr {
                    ty: Type::Range,
                    kind: ExprKind::Range(Box::new(a), Box::new(b)),
                }
            }
            A::Array(values) => {
                let context = expected.and_then(|t| self.types.array(t));
                let hint = context.map(|a| a.elem);
                let mut checked = Vec::new();
                let mut elem = hint;
                for value in values {
                    let value = self.expr(value, elem)?;
                    elem = Some(value.ty);
                    checked.push(value);
                }
                let elem = elem.ok_or_else(|| {
                    Error::new(
                        "E0220",
                        e.span,
                        "an empty array needs an explicit type, for example [i32; 0]",
                    )
                })?;
                let ty = self.types.array_type(elem, values.len(), e.span)?;
                let storage = self.storage(ty).unwrap();
                Expr {
                    ty,
                    kind: ExprKind::Array(checked, storage),
                }
            }
            A::Repeat(value, len) => {
                let hint = expected.and_then(|t| self.types.array(t)).map(|a| a.elem);
                let value = self.expr(value, hint)?;
                let ty = self.types.array_type(value.ty, *len, e.span)?;
                let storage = self.storage(ty).unwrap();
                Expr {
                    ty,
                    kind: ExprKind::Repeat(Box::new(value), storage),
                }
            }
            A::Index(base, index) => {
                let base = self.expr(base, None)?;
                if !matches!(base.ty, Type::Array(_)) {
                    return Err(Error::new(
                        "E0221",
                        e.span,
                        "indexing requires a fixed array",
                    ));
                }
                let item = self.types.array(base.ty).unwrap().elem;
                let index = self.expr(index, Some(Type::I32))?;
                Expr {
                    ty: item,
                    kind: ExprKind::Index(Box::new(base), Box::new(index)),
                }
            }
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
                } else if let Some((index, ty)) = self.global(n) {
                    Expr {
                        ty,
                        kind: ExprKind::Function(index),
                    }
                } else {
                    return Err(Error::new("E0205", e.span, format!("unknown name '{n}'")));
                }
            }
            A::TestTarget(n) => {
                let (index, ty) = self
                    .global(n)
                    .ok_or_else(|| Error::new("E0231", e.span, "missing annotated test target"))?;
                Expr {
                    ty,
                    kind: ExprKind::Function(index),
                }
            }
            A::Member(base, name) => {
                if let Some(record) = self.hint(base).and_then(|ty| self.types.record(ty)) {
                    let field =
                        record
                            .fields
                            .iter()
                            .find(|f| f.name == *name)
                            .ok_or_else(|| {
                                Error::new(
                                    "E0241",
                                    e.span,
                                    format!("{} has no field '{name}'", record.name),
                                )
                            })?;
                    let (ty, offset) = (field.ty, field.offset);
                    let base = self.expr(base, None)?;
                    Expr {
                        ty,
                        kind: ExprKind::Field(Box::new(base), offset),
                    }
                } else {
                    let (index, ty) = self.member(base, name, e.span)?;
                    Expr {
                        ty,
                        kind: ExprKind::Function(index),
                    }
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
            A::Call(callee, args) if matches!(&callee.kind,A::Name(n) if n=="len") => {
                if args.len() != 1 {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "len expects one array or array iterator",
                    ));
                }
                let base = self.expr(&args[0], None)?;
                let len = self
                    .types
                    .array(base.ty)
                    .ok_or_else(|| {
                        Error::new("E0221", e.span, "len requires an array or array iterator")
                    })?
                    .len;
                Expr {
                    ty: Type::I32,
                    kind: ExprKind::Len(Box::new(base), len),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind,A::Member(base,name) if matches!(name.as_str(),"iter"|"len") && !self.module_alias(base)) =>
            {
                if !args.is_empty() {
                    return Err(Error::new("E0207", e.span, "iter/len takes no arguments"));
                }
                let A::Member(base, name) = &callee.kind else {
                    unreachable!()
                };
                let base = self.expr(base, None)?;
                let array = self.types.array(base.ty).ok_or_else(|| {
                    Error::new(
                        "E0221",
                        e.span,
                        "iter/len requires an array or array iterator",
                    )
                })?;
                if name == "len" {
                    let len = array.len;
                    Expr {
                        ty: Type::I32,
                        kind: ExprKind::Len(Box::new(base), len),
                    }
                } else {
                    let id = match base.ty {
                        Type::Array(i) | Type::Iterator(i) => i,
                        _ => unreachable!(),
                    };
                    Expr {
                        ty: Type::Iterator(id),
                        kind: ExprKind::Iterator(Box::new(base)),
                    }
                }
            }
            A::Call(callee, args) if matches!(&callee.kind,A::Name(n) if n=="assert") => {
                if args.len() != 1 {
                    return Err(Error::new("E0207", e.span, "assert expects one bool"));
                }
                let condition = self.expr(&args[0], Some(Type::Bool))?;
                Expr {
                    ty: Type::Unit,
                    kind: ExprKind::Assert(Box::new(condition)),
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
                    let ty = ty
                        .map(|ty| resolve_type(self.types, self.symbols, ty, s.span))
                        .transpose()?;
                    let x = self.expr(init, ty)?;
                    self.bind(name, x.ty, *mutable, s.span)?;
                    let b = self.lookup(name).unwrap();
                    (self.store_value(&b, x), false)
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
                    let x = self.expr(value, Some(b.ty))?;
                    (self.store_value(&b, x), false)
                }
                ast::StmtKind::AssignIndex(target, value) => {
                    if let ast::ExprKind::Member(base, name) = &target.kind {
                        let ast::ExprKind::Name(local) = &base.kind else {
                            return Err(Error::new(
                                "E0241",
                                target.span,
                                "assign through a mutable local struct name",
                            ));
                        };
                        let b = self.lookup(local).ok_or_else(|| {
                            Error::new("E0205", base.span, format!("unknown name '{local}'"))
                        })?;
                        if !b.mutable {
                            return Err(Error::new(
                                "E0208",
                                target.span,
                                format!("'{local}' is immutable; use 'var' to change its fields"),
                            ));
                        }
                        let field = self
                            .types
                            .record(b.ty)
                            .and_then(|r| r.fields.iter().find(|f| f.name == *name))
                            .ok_or_else(|| {
                                Error::new("E0241", target.span, format!("no field '{name}'"))
                            })?;
                        let (offset, ty) = (field.offset, field.ty);
                        let base = self.expr(base, Some(b.ty))?;
                        let value = self.expr(value, Some(ty))?;
                        result.push(Stmt::StoreField(base, offset, value));
                        continue;
                    }
                    let ast::ExprKind::Index(base, index) = &target.kind else {
                        return Err(Error::new(
                            "E0221",
                            target.span,
                            "assignment target must be an array element",
                        ));
                    };
                    let ast::ExprKind::Name(name) = &base.kind else {
                        return Err(Error::new(
                            "E0221",
                            target.span,
                            "assign through a mutable local array name",
                        ));
                    };
                    let b = self.lookup(name).ok_or_else(|| {
                        Error::new("E0205", base.span, format!("unknown array '{name}'"))
                    })?;
                    if !b.mutable {
                        return Err(Error::new(
                            "E0208",
                            target.span,
                            format!("'{name}' is immutable; use 'var' to change its elements"),
                        ));
                    }
                    if !matches!(b.ty, Type::Array(_)) {
                        return Err(Error::new(
                            "E0221",
                            target.span,
                            "assignment requires a mutable array",
                        ));
                    }
                    let ty = self.types.array(b.ty).unwrap().elem;
                    let base = self.expr(base, Some(b.ty))?;
                    let index = self.expr(index, Some(Type::I32))?;
                    let value = self.expr(value, Some(ty))?;
                    (Stmt::StoreIndex(base, index, value), false)
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
                ast::StmtKind::Expr(e) | ast::StmtKind::Tail(e) => {
                    let mut x = self.expr(e, None)?;
                    if self.testing && x.ty == Type::Bool {
                        x = Expr {
                            ty: Type::Unit,
                            kind: ExprKind::Assert(Box::new(x)),
                        };
                    }
                    if x.ty != Type::Unit {
                        return Err(Error::new(
                            "E0209",
                            s.span,
                            "unused value; assign with ':=' or return it",
                        ));
                    }
                    (Stmt::Expr(x), false)
                }
                ast::StmtKind::Case(_, _) => {
                    return Err(Error::new(
                        "E0231",
                        s.span,
                        "argument cases require an attached @test block",
                    ))
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
                ast::StmtKind::For(name, source, body) => {
                    let source = self.expr(source, None)?;
                    let item = if source.ty == Type::Range {
                        Type::I32
                    } else {
                        self.types
                            .array(source.ty)
                            .ok_or_else(|| {
                                Error::new(
                                    "E0222",
                                    s.span,
                                    "for requires a range, array or array iterator",
                                )
                            })?
                            .elem
                    };
                    self.scopes.push(HashMap::new());
                    let binding = self.bind(name, item, false, s.span)?;
                    let source_slot = self.slot();
                    let cursor = self.slot();
                    let end = self.slot();
                    let storage = self.storage(source.ty);
                    self.loops += 1;
                    let (body, _) = self.block(body)?;
                    self.loops -= 1;
                    self.scopes.pop();
                    (
                        Stmt::For {
                            source,
                            source_slot,
                            cursor,
                            end,
                            binding,
                            storage,
                            body,
                        },
                        false,
                    )
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
    check_mode(program, Mode::Program)
}
pub fn check_mode(program: ast::Program, mode: Mode) -> Result<Program, Error> {
    if let Some(import) = program.imports.first() {
        return Err(Error::new(
            "E0400",
            import.span,
            "imports require a source path; use the file-based module loader",
        ));
    }
    check_modules(
        vec![Module {
            functions: program.functions,
            imports: Vec::new(),
            records: program.records,
        }],
        program.types,
        mode,
    )
}

pub(crate) fn check_modules(
    mut modules: Vec<Module>,
    mut types: Types,
    mode: Mode,
) -> Result<Program, Error> {
    if mode == Mode::Program {
        for module in &mut modules {
            module.functions.retain(|f| f.test.is_none());
        }
    }
    let mut tests = Vec::new();
    let mut symbols: Vec<Symbols> = (0..modules.len()).map(|_| Symbols::default()).collect();
    // Reserve nominal types and all file namespaces before resolving signatures.
    for (module, input) in modules.iter().enumerate() {
        let mut names = std::collections::HashSet::new();
        for record in &input.records {
            if reserved(&record.name) {
                return Err(Error::new(
                    "E0200",
                    record.span,
                    format!("'{}' is reserved", record.name),
                )
                .in_file(module));
            }
            if !names.insert(record.name.clone()) {
                return Err(
                    Error::new("E0203", record.span, "duplicate struct name").in_file(module)
                );
            }
            let ty = types.declare_record(module, record.name.clone());
            symbols[module]
                .records
                .insert(record.name.clone(), (ty, record.public));
        }
        for f in &input.functions {
            if reserved(&f.name) {
                return Err(
                    Error::new("E0200", f.span, format!("'{}' is reserved", f.name))
                        .in_file(module),
                );
            }
            if !names.insert(f.name.clone()) {
                return Err(Error::new(
                    "E0203",
                    f.span,
                    format!("duplicate function or type '{}'", f.name),
                )
                .in_file(module));
            }
        }
        for import in &input.imports {
            if reserved(&import.name) || !identifier(&import.name) {
                return Err(Error::new(
                    "E0401",
                    import.span,
                    "invalid or reserved module alias; choose a name with 'as'",
                )
                .in_file(module));
            }
            if names.contains(&import.name)
                || symbols[module]
                    .imports
                    .insert(import.name.clone(), import.target)
                    .is_some()
            {
                return Err(Error::new(
                    "E0402",
                    import.span,
                    format!("duplicate module or declaration name '{}'", import.name),
                )
                .in_file(module));
            }
        }
    }
    for (module, input) in modules.iter().enumerate() {
        for record in &input.records {
            let ty = symbols[module].records[&record.name].0;
            let mut fields = Vec::new();
            let mut offset = 0;
            let mut alignment = 1;
            let mut seen = std::collections::HashSet::new();
            for field in &record.fields {
                if !seen.insert(field.name.clone()) {
                    return Err(
                        Error::new("E0240", field.span, "duplicate struct field").in_file(module)
                    );
                }
                let ty = resolve_type(&mut types, &symbols, field.ty, field.span)
                    .map_err(|e| e.in_file(module))?;
                let size = match ty {
                    Type::Bool => 1,
                    Type::I32 => 4,
                    Type::I64 => 8,
                    _ => {
                        return Err(Error::new(
                            "E0240",
                            field.span,
                            "struct fields currently support i32, i64 and bool",
                        )
                        .in_file(module))
                    }
                };
                offset = (offset + size - 1) & !(size - 1);
                fields.push(ast::RecordField {
                    name: field.name.clone(),
                    ty,
                    offset,
                });
                offset += size;
                alignment = alignment.max(size);
                if offset > 65536 {
                    return Err(Error::new(
                        "E0240",
                        record.span,
                        "a struct may occupy at most 65536 bytes",
                    )
                    .in_file(module));
                }
            }
            let bytes = (offset.max(1) + alignment - 1) & !(alignment - 1);
            types.define_record(ty, fields, bytes);
        }
    }
    let mut index = 0;
    for (module, input) in modules.iter_mut().enumerate() {
        let mut test_names = std::collections::HashSet::new();
        for f in &mut input.functions {
            for p in &mut f.params {
                p.ty = resolve_type(&mut types, &symbols, p.ty, p.span)
                    .map_err(|e| e.in_file(module))?;
            }
            f.ret =
                resolve_type(&mut types, &symbols, f.ret, f.span).map_err(|e| e.in_file(module))?;
            if let Some(name) = &f.test {
                if !f.params.is_empty() || f.ret != Type::Unit {
                    return Err(Error::new(
                        "E0230",
                        f.span,
                        "a @test function must take no arguments and return ()",
                    )
                    .in_file(module));
                }
                if !test_names.insert(name.clone()) {
                    return Err(Error::new(
                        "E0230",
                        f.span,
                        format!("duplicate test name '{name}'"),
                    )
                    .in_file(module));
                }
                tests.push(TestCase {
                    name: name.clone(),
                    file: module,
                    function: index,
                });
            }
            if types.storage_bytes(f.ret).is_some() {
                return Err(Error::new("E0223",f.span,"returning aggregate values is not supported yet; return scalar results from aggregate parameters").in_file(module));
            }
            let ty = types.function(f.params.iter().map(|p| p.ty).collect(), f.ret);
            symbols[module].functions.insert(
                f.name.clone(),
                Global {
                    index,
                    ty,
                    public: f.public,
                },
            );
            index += 1;
        }
    }
    let span = Span { start: 0, end: 0 };
    let entry = symbols.first().and_then(|s| s.functions.get("main"));
    if entry.is_none() && mode == Mode::Program {
        return Err(Error::new(
            "E0212",
            span,
            "root module needs 'fn main()' or 'fn main() -> i32'",
        ));
    }
    if let Some(entry) = entry {
        let sig = types.signature(entry.ty).unwrap();
        if !sig.params.is_empty() || !matches!(sig.ret, Type::I32 | Type::Unit) {
            return Err(Error::new(
                "E0212",
                modules[0].functions[entry.index].span,
                "main must take no arguments and return i32 or ()",
            ));
        }
    }
    let main = entry.map(|e| e.index);
    let mut checked = Vec::new();
    for (module, input) in modules.into_iter().enumerate() {
        for f in input.functions {
            let function = (|| {
                let mut c = Checker {
                    module,
                    symbols: &symbols,
                    types: &mut types,
                    scopes: vec![HashMap::new()],
                    slots: 0,
                    ret: f.ret,
                    loops: 0,
                    testing: f.test.is_some(),
                };
                let mut param_slots = Vec::new();
                let mut param_storage = Vec::new();
                for p in &f.params {
                    param_slots.push(c.bind(&p.name, p.ty, false, p.span)?);
                    param_storage.push(c.lookup(&p.name).unwrap().storage);
                }
                let (body, returns) = c.statements(&f.body)?;
                if f.ret != Type::Unit && !returns {
                    return Err(Error::new(
                        "E0213",
                        f.span,
                        format!(
                            "function '{}' must return {} on every path",
                            f.name,
                            c.types.name(f.ret)
                        ),
                    ));
                }
                Ok(Function {
                    name: format!("m{module}.{}", f.name),
                    params: f.params.iter().map(|p| p.ty).collect(),
                    slots: c.slots,
                    body,
                    param_slots,
                    param_storage,
                })
            })()
            .map_err(|e: Error| e.in_file(module))?;
            checked.push(function);
        }
    }
    Ok(Program {
        types,
        functions: checked,
        main,
        tests,
    })
}

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn lookup_type(symbols: &[Symbols], module: usize, name: &str, span: Span) -> Result<Type, Error> {
    let parts: Vec<_> = name.split('.').collect();
    let (owner, local, imported) = match parts.as_slice() {
        [local] => (module, *local, false),
        [alias, local] => {
            let owner = symbols[module].imports.get(*alias).ok_or_else(|| {
                Error::new("E0103", span, format!("unknown type namespace '{alias}'"))
            })?;
            (*owner, *local, true)
        }
        _ => {
            return Err(Error::new(
                "E0103",
                span,
                "a type path must be Type or module.Type",
            ))
        }
    };
    let (ty, public) = symbols[owner]
        .records
        .get(local)
        .ok_or_else(|| Error::new("E0103", span, format!("unknown type '{name}'")))?;
    if imported && !*public {
        return Err(Error::new(
            "E0403",
            span,
            format!("type '{name}' is private; export it with 'pub struct'"),
        ));
    }
    Ok(*ty)
}
fn resolve_type(
    types: &mut Types,
    symbols: &[Symbols],
    ty: Type,
    span: Span,
) -> Result<Type, Error> {
    if let Some(named) = types.named_ref(ty).cloned() {
        return lookup_type(symbols, named.module, &named.name, span);
    }
    if let Some(signature) = types.signature(ty).cloned() {
        let params = signature
            .params
            .into_iter()
            .map(|t| resolve_type(types, symbols, t, span))
            .collect::<Result<Vec<_>, _>>()?;
        let ret = resolve_type(types, symbols, signature.ret, span)?;
        return Ok(types.function(params, ret));
    }
    Ok(ty)
}
