use crate::{
    syntax::{self as ast, Type, Types},
    Error, Span,
};
use std::collections::HashMap;
mod collections;
mod enums;
mod ffi;
mod regions;

pub struct Program {
    pub types: Types,
    pub functions: Vec<Function>,
    pub main: Option<usize>,
    pub tests: Vec<TestCase>,
    pub float_formatter: Option<usize>,
    pub sys_types: Option<SysTypes>,
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
    pub filesystem: bool,
    pub functions: Vec<ast::Function>,
    pub imports: Vec<Import>,
    pub records: Vec<ast::Record>,
    pub enums: Vec<ast::Enumeration>,
    pub standard_sys: bool,
}
pub struct Import {
    pub name: String,
    pub target: usize,
    pub span: Span,
}
struct Global {
    requires_filesystem: bool,
    index: usize,
    ty: Type,
    public: bool,
}
#[derive(Default)]
struct Symbols {
    filesystem: bool,
    functions: HashMap<String, Global>,
    imports: HashMap<String, usize>,
    records: HashMap<String, (Type, bool)>,
    standard_sys: bool,
}
#[derive(Hash)]
pub struct Function {
    pub ret: Type,
    pub external: Option<(String, String)>,
    pub name: String,
    pub params: Vec<Type>,
    pub slots: usize,
    pub body: Vec<Stmt>,
    pub param_slots: Vec<usize>,
    pub param_storage: Vec<Option<usize>>,
    pub return_slot: Option<usize>,
    /// Owned aggregate storage and immutable string handles, including temporaries.
    pub owned_storage: Vec<(usize, Type)>,
    pub uses_float_format: bool,
}
#[derive(Hash)]
pub enum Stmt {
    StorePlace(Expr, Expr),
    Match {
        source: Expr,
        slot: usize,
        storage: Option<usize>,
        arms: Vec<EnumArm>,
    },
    Store(usize, Expr),
    StoreAggregate(usize, usize, Expr),
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
    DropStorage(Vec<(usize, Type)>),
    Break,
    Continue,
}
#[derive(Hash)]
pub struct EnumArm {
    pub tag: Option<i32>,
    pub bindings: Vec<EnumBinding>,
    pub body: Vec<Stmt>,
}
#[derive(Hash)]
pub struct EnumBinding {
    pub ty: Type,
    pub offset: usize,
    pub slot: usize,
    pub storage: Option<usize>,
}
#[derive(Hash)]
pub struct Expr {
    pub ty: Type,
    pub kind: ExprKind,
}
#[derive(Hash)]
pub enum ExprKind {
    ManagedString(Box<Expr>, usize, bool),
    ReferenceAddress(usize),
    LocalAddress(usize),
    Deref(Box<Expr>),
    Offset(Box<Expr>, usize),
    ElementAddress(Box<Expr>, Box<Expr>),
    Clone(Box<Expr>, usize),
    Dynamic(Vec<Expr>, usize),
    ScratchAlloc {
        area: usize,
        cursor: usize,
        limit: usize,
        count: Box<Expr>,
        storage: usize,
    },
    DynamicOp(DynamicOp, Box<Expr>, Option<Box<Expr>>),
    IntOp(IntOp, Box<Expr>, Option<Box<Expr>>),
    Enum {
        tag: i32,
        fields: Vec<(usize, Expr)>,
        storage: Option<usize>,
    },
    EnumFromInt(Box<Expr>),
    Number(i64),
    Float(u64),
    FloatOp(FloatOp, Box<Expr>),
    Bool(bool),
    String(String),
    StringView(Box<Expr>),
    StringOp(StringOp, Vec<Expr>),
    Local(usize),
    Function(usize),
    Callback(usize),
    Call(Box<Expr>, Vec<Expr>, Option<usize>),
    SysCall(SysOp, Vec<Expr>, Option<usize>),
    StaticBytes {
        module: usize,
        site: usize,
        bytes: usize,
        platform: String,
    },
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SysOp {
    ReadText,
    WriteText,
    WriteStderr,
    AppendText,
    Mkdir,
    Rename,
    RemoveFile,
    RemoveDir,
    Metadata,
    Pid,
    ArgCount,
    Arg,
    WriteBytes,
    MakeExecutable,
    DecodeUtf8,
    SameFile,
}
#[derive(Clone, Copy, Hash)]
pub enum DynamicOp {
    Len,
    Capacity,
    Push,
    Pop,
    Reserve,
    Clear,
}
#[derive(Clone, Copy, Hash)]
pub enum IntOp {
    And,
    Or,
    Xor,
    Not,
    Shl,
    Shr,
    WrappingAdd,
    WrappingSub,
    WrappingMul,
}
#[derive(Clone, Copy)]
pub struct SysTypes {
    pub status: Type,
    pub text: Type,
    pub metadata: Type,
}
#[derive(Clone, Copy, Hash)]
pub enum FloatOp {
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Round,
    RoundEven,
    Trunc,
    IsNan,
    IsFinite,
    IsInfinite,
    SignNegative,
    ToBits,
    FromBits,
}
#[derive(Clone, Copy, Hash)]
pub enum StringOp {
    Len,
    RuneLen,
    ByteAt,
    RuneAt,
    Slice,
    ToString,
    ToRune,
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    slot: usize,
    mutable: bool,
    storage: Option<usize>,
}
pub struct BindingInfo {
    pub file: usize,
    pub span: Span,
    pub name: String,
    pub ty: Type,
}

struct Checker<'a> {
    in_unsafe: bool,
    reports: Option<&'a mut Vec<BindingInfo>>,
    module: usize,
    symbols: &'a [Symbols],
    types: &'a mut Types,
    scopes: Vec<HashMap<String, Binding>>,
    regions: regions::Regions,
    slots: usize,
    owned_storage: Vec<(usize, Type)>,
    ret: Type,
    loops: usize,
    loop_owned_starts: Vec<usize>,
    testing: bool,
    needs_float_format: bool,
}

fn reserved(name: &str) -> bool {
    ffi::scalar(name).is_some()
        || matches!(name, "unsafe" | "extern" | "capability")
        || matches!(
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
                | "string"
                | "rune"
                | "f32"
                | "f64"
                | "enum"
                | "match"
                | "mut"
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
            let slot = self.slots - 1;
            self.owned_storage.push((slot, ty));
            slot
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
    fn bound_name(&self, name: &str) -> bool {
        self.lookup(name).is_some() || self.symbols[self.module].functions.contains_key(name)
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
        self.check_capability(function, span)?;
        Ok((function.index, function.ty))
    }
    fn check_capability(&self, function: &Global, span: Span) -> Result<(), Error> {
        if function.requires_filesystem && !self.symbols[self.module].filesystem {
            return Err(Error::new(
                "E0292",
                span,
                "this function requires capability filesystem at the top of this module",
            ));
        }
        Ok(())
    }

    fn check_record_access(&self, ty: Type, span: Span) -> Result<(), Error> {
        if self
            .types
            .record(ty)
            .is_some_and(|r| r.opaque && r.module != self.module)
        {
            return Err(Error::new(
                "E0292",
                span,
                "an opaque struct can only be constructed or opened by its defining module",
            ));
        }
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned()
    }
    fn bind(&mut self, name: &str, ty: Type, mutable: bool, span: Span) -> Result<usize, Error> {
        self.bind_local(name, ty, mutable, span, true)
    }
    fn bind_local(
        &mut self,
        name: &str,
        ty: Type,
        mutable: bool,
        span: Span,
        owner: bool,
    ) -> Result<usize, Error> {
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
        if let Some(reports) = &mut self.reports {
            reports.push(BindingInfo {
                file: self.module,
                span,
                name: name.into(),
                ty,
            });
        }
        let slot = self.slot();
        let storage = if owner { self.storage(ty) } else { None };
        if owner && matches!(ty, Type::String | Type::Bytes | Type::Runes) {
            self.owned_storage.push((slot, ty));
        }
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
            ast::ExprKind::Variant(_) => None,
            ast::ExprKind::Member(_, _)
                if self.enum_variant(expr, None).ok().flatten().is_some() =>
            {
                self.enum_variant(expr, None)
                    .ok()
                    .flatten()
                    .map(|(ty, _)| ty)
            }
            ast::ExprKind::String(_) => Some(Type::String),
            ast::ExprKind::Rune(_) => Some(Type::Rune),
            ast::ExprKind::TestTarget(n) => self.global(n).map(|(_, ty)| ty),
            ast::ExprKind::Name(n) => self
                .lookup(n)
                .map(|b| self.types.value_type(b.ty))
                .or_else(|| self.global(n).map(|(_, ty)| ty)),
            ast::ExprKind::Member(base, name)
                if name == "len"
                    && self.hint(base).is_some_and(|ty| {
                        matches!(ty, Type::String | Type::Bytes | Type::Runes)
                            || self.types.array(ty).is_some()
                            || self.types.dynamic_elem(ty).is_some()
                    }) =>
            {
                Some(Type::I32)
            }
            ast::ExprKind::Member(base, name) => self
                .hint(base)
                .and_then(|ty| self.types.record(ty))
                .and_then(|r| r.fields.iter().find(|f| f.name == *name))
                .map(|f| f.ty)
                .or_else(|| self.member(base, name, expr.span).ok().map(|(_, ty)| ty)),
            ast::ExprKind::Record(name, _) => self.constructor_type(name).ok(),
            ast::ExprKind::Call(callee, _) => {
                if self.integer_method(callee).is_some() {
                    if let ast::ExprKind::Member(base, _) = &callee.kind {
                        return self.hint(base).or_else(|| {
                            matches!(base.kind, ast::ExprKind::Number(_)).then_some(Type::I32)
                        });
                    }
                }
                if let Some(op) = self.dynamic_method(callee) {
                    if let ast::ExprKind::Member(base, _) = &callee.kind {
                        return Some(match op {
                            DynamicOp::Len | DynamicOp::Capacity => Type::I32,
                            DynamicOp::Pop => self.types.dynamic_elem(self.hint(base)?)?,
                            _ => Type::Unit,
                        });
                    }
                }
                if let Some((ty, _)) = self.enum_variant(callee, None).ok().flatten() {
                    return Some(ty);
                }
                if let Some(ty) = self.enum_type_expr(callee).ok().flatten() {
                    return Some(ty);
                }
                if let ast::ExprKind::Name(n) = &callee.kind {
                    if self.bound_name(n) {
                        return self
                            .hint(callee)
                            .and_then(|t| self.types.signature(t))
                            .map(|s| s.ret);
                    }
                    if let Some(ty) =
                        ffi::scalar(n).or_else(|| n.strip_prefix("load_").and_then(ffi::scalar))
                    {
                        return Some(ty);
                    }
                    if matches!(n.as_str(), "addr" | "cstr") {
                        return Some(Type::Ptr);
                    }
                    if matches!(n.as_str(), "size_of" | "align_of") {
                        return Some(Type::Usize);
                    }
                    if n == "i32" {
                        return Some(Type::I32);
                    }
                    if n == "i64" {
                        return Some(Type::I64);
                    }
                    if n == "len" {
                        return Some(Type::I32);
                    }
                    if n == "rune" {
                        return Some(Type::Rune);
                    }
                    if n == "f32" {
                        return Some(Type::F32);
                    }
                    if n == "f64" {
                        return Some(Type::F64);
                    }
                }
                if let ast::ExprKind::Member(base, name) = &callee.kind {
                    if name == "from_bits" {
                        if matches!(&base.kind,ast::ExprKind::Name(n) if n=="f32") {
                            return Some(Type::F32);
                        }
                        if matches!(&base.kind,ast::ExprKind::Name(n) if n=="f64") {
                            return Some(Type::F64);
                        }
                    }
                    if let Some(ty) = self.receiver_type(base) {
                        if ty.float() {
                            if matches!(
                                name.as_str(),
                                "is_nan" | "is_finite" | "is_infinite" | "is_sign_negative"
                            ) {
                                return Some(Type::Bool);
                            }
                            if name == "to_bits" {
                                return Some(if ty == Type::F32 {
                                    Type::I32
                                } else {
                                    Type::I64
                                });
                            }
                            if name != "str" {
                                return self.hint(base);
                            }
                        }
                        if name == "str" {
                            return Some(Type::String);
                        }
                        if matches!(ty, Type::String | Type::Bytes | Type::Runes) {
                            match name.as_str() {
                                "len" => return Some(Type::I32),
                                "bytes" if ty == Type::String => return Some(Type::Bytes),
                                "runes" if ty == Type::String => return Some(Type::Runes),
                                _ => {}
                            }
                        }
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
            ast::ExprKind::Index(base, index) => self.hint(base).and_then(|t| {
                if t == Type::String && self.hint(index) == Some(Type::Range) {
                    Some(Type::String)
                } else {
                    match t {
                        Type::String | Type::Bytes => Some(Type::I32),
                        Type::Runes => Some(Type::Rune),
                        _ => self
                            .types
                            .dynamic_elem(t)
                            .or_else(|| self.types.array(t).map(|a| a.elem)),
                    }
                }
            }),
            ast::ExprKind::Range(_, _) => Some(Type::Range),
            ast::ExprKind::Binary(op, a, b) if matches!(*op, "+" | "-" | "*" | "/" | "%") => {
                self.hint(a).or_else(|| self.hint(b))
            }
            _ => None,
        }
    }
    fn number(&self, value: i128, ty: Type, span: Span) -> Result<Expr, Error> {
        if ty.float() {
            return self.float(&value.to_string(), ty, span);
        }
        let valid = match ty {
            Type::U8 => u8::try_from(value).is_ok(),
            Type::U16 => u16::try_from(value).is_ok(),
            Type::U32 => u32::try_from(value).is_ok(),
            Type::Usize if self.types.pointer_bytes() == 4 => u32::try_from(value).is_ok(),
            Type::U64 | Type::Usize => u64::try_from(value).is_ok(),
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
    fn float(&self, spelling: &str, ty: Type, span: Span) -> Result<Expr, Error> {
        let bits = match ty {
            Type::F32 => spelling
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite())
                .map(|n| u64::from(n.to_bits())),
            Type::F64 => spelling
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .map(f64::to_bits),
            _ => return Err(mismatch(self.types, span, ty, Type::F64)),
        }
        .ok_or_else(|| {
            Error::new(
                "E0260",
                span,
                format!(
                    "floating literal '{spelling}' is not finite in {}",
                    ty.name()
                ),
            )
        })?;
        Ok(Expr {
            ty,
            kind: ExprKind::Float(bits),
        })
    }
    fn float_literal(expr: &ast::Expr) -> bool {
        match &expr.kind {
            ast::ExprKind::Float(_) => true,
            ast::ExprKind::Unary("-", value) => Self::float_literal(value),
            ast::ExprKind::Binary(op, a, b) if matches!(*op, "+" | "-" | "*" | "/" | "%") => {
                Self::float_literal(a) || Self::float_literal(b)
            }
            ast::ExprKind::Call(callee, args) if args.is_empty() => {
                matches!(&callee.kind, ast::ExprKind::Member(base, name)
                    if matches!(name.as_str(), "abs"|"sqrt"|"floor"|"ceil"|"round"|"round_ties_even"|"trunc") && Self::float_literal(base))
            }
            _ => false,
        }
    }
    fn receiver_type(&self, expr: &ast::Expr) -> Option<Type> {
        self.hint(expr)
            .or_else(|| Self::float_literal(expr).then_some(Type::F64))
    }
    fn sys_signature(&self, name: &str) -> Option<(SysOp, usize, Type)> {
        let symbols = &self.symbols[self.module];
        if !symbols.standard_sys {
            return None;
        }
        let (op, count, result) = match name {
            "__sys_read_text" => (SysOp::ReadText, 1, "TextResult"),
            "__sys_write_text" => (SysOp::WriteText, 2, "Status"),
            "__sys_write_stderr" => (SysOp::WriteStderr, 1, "Status"),
            "__sys_append_text" => (SysOp::AppendText, 2, "Status"),
            "__sys_mkdir" => (SysOp::Mkdir, 1, "Status"),
            "__sys_rename" => (SysOp::Rename, 2, "Status"),
            "__sys_remove_file" => (SysOp::RemoveFile, 1, "Status"),
            "__sys_remove_dir" => (SysOp::RemoveDir, 1, "Status"),
            "__sys_metadata" => (SysOp::Metadata, 1, "Metadata"),
            "__sys_pid" => return Some((SysOp::Pid, 0, Type::I32)),
            "__sys_arg_count" => return Some((SysOp::ArgCount, 0, Type::I32)),
            "__sys_arg" => return Some((SysOp::Arg, 1, Type::String)),
            "__sys_write_bytes" | "__sys_write_u8" => (SysOp::WriteBytes, 2, "Status"),
            "__sys_make_executable" => (SysOp::MakeExecutable, 1, "Status"),
            "__sys_decode_utf8" | "__sys_decode_u8" => (SysOp::DecodeUtf8, 1, "TextResult"),
            "__sys_same_file" => return Some((SysOp::SameFile, 2, Type::Bool)),
            _ => return None,
        };
        Some((op, count, symbols.records[result].0))
    }
    fn expr(&mut self, e: &ast::Expr, expected: Option<Type>) -> Result<Expr, Error> {
        let value = self.unmanaged_expr(e, expected)?;
        if !matches!(value.ty, Type::String | Type::Bytes | Type::Runes) {
            return Ok(value);
        }
        // Even a read needs a retained snapshot before later operands can mutate
        // its source. Fresh runtime results already carry their first ownership.
        let fresh = match &value.kind {
            ExprKind::Binary("+", _, _)
            | ExprKind::Call(_, _, _)
            | ExprKind::SysCall(_, _, _)
            | ExprKind::DynamicOp(DynamicOp::Pop, _, _)
            | ExprKind::StringOp(StringOp::Slice, _) => true,
            ExprKind::StringOp(StringOp::ToString, args) => {
                !matches!(args[0].ty, Type::String | Type::Bool)
            }
            _ => false,
        };
        let slot = self.slot();
        let ty = value.ty;
        self.owned_storage.push((slot, ty));
        Ok(Expr {
            ty,
            kind: ExprKind::ManagedString(Box::new(value), slot, fresh),
        })
    }
    fn unmanaged_expr(&mut self, e: &ast::Expr, expected: Option<Type>) -> Result<Expr, Error> {
        use ast::ExprKind as A;
        let typed = match &e.kind {
            A::Call(callee, args) if matches!(&callee.kind, A::Name(n) if !self.bound_name(n) && (matches!(n.as_str(), "ptr"|"addr"|"ref_at"|"ffi_fn"|"size_of"|"align_of"|"cstr"|"zeroed"|"callback") || n.strip_prefix("load_").and_then(ffi::scalar).is_some())) =>
            {
                let A::Name(name) = &callee.kind else {
                    unreachable!()
                };
                self.raw_call(name, args, expected, e.span)?
            }
            A::Call(callee, args) if matches!(&callee.kind, A::Name(n) if n == "static_bytes") => {
                self.require_unsafe(e.span)?;
                let [platform, length] = args.as_slice() else {
                    return Err(Error::new(
                        "E0291",
                        e.span,
                        "static_bytes expects a native platform string and a constant byte count",
                    ));
                };
                let A::String(platform) = &platform.kind else {
                    return Err(Error::new(
                        "E0291",
                        platform.span,
                        "expected a platform string",
                    ));
                };
                if !matches!(platform.as_str(), "linux" | "macos" | "native") {
                    return Err(Error::new(
                        "E0291",
                        e.span,
                        "static storage platform must be linux, macos or native",
                    ));
                }
                let A::Number(bytes @ 1..=16777216) = length.kind else {
                    return Err(Error::new(
                        "E0291",
                        length.span,
                        "static byte count must be a constant between 1 and 16777216",
                    ));
                };
                Expr {
                    ty: Type::Ptr,
                    kind: ExprKind::StaticBytes {
                        module: self.module,
                        site: e.span.start,
                        bytes: bytes as usize,
                        platform: platform.clone(),
                    },
                }
            }
            A::Borrow(base, mutable) => {
                let Some((inner, allowed_mut)) = expected.and_then(|t| self.types.referenced(t))
                else {
                    return Err(Error::new(
                        "E0281",
                        e.span,
                        "borrows are currently allowed only as function arguments",
                    ));
                };
                if *mutable != allowed_mut {
                    return Err(Error::new(
                        "E0281",
                        e.span,
                        "use & for a shared argument or &mut for a mutable argument",
                    ));
                }
                let (ty, mut address) =
                    if let Some(view) = self.scratch_slice(base).filter(|_| !mutable) {
                        (
                            view.ty,
                            Expr {
                                ty: view.ty,
                                kind: ExprKind::Local(view.slot),
                            },
                        )
                    } else {
                        self.place(base, *mutable)?
                    };
                if ty != inner {
                    return Err(mismatch(self.types, e.span, inner, ty));
                }
                address.ty = expected.unwrap();
                address
            }
            A::Call(callee, args) if self.dynamic_method(callee).is_some() => {
                self.check_dynamic_method(callee, args, e.span)?
            }
            A::Call(callee, args) if self.integer_method(callee).is_some() => {
                self.check_integer_method(callee, args, e.span)?
            }
            A::Call(callee, args) if self.enum_variant(callee, expected)?.is_some() => {
                self.enum_construct(callee, args, expected)?
            }
            A::Member(_, _) | A::Variant(_) if self.enum_variant(e, expected)?.is_some() => {
                self.enum_construct(e, &[], expected)?
            }
            A::Variant(_) => {
                return Err(Error::new(
                    "E0271",
                    e.span,
                    "a short enum variant needs an enum type context",
                ))
            }
            A::Call(callee, args) if self.enum_type_expr(callee)?.is_some() => {
                let ty = self.enum_type_expr(callee)?.unwrap();
                if !self.types.scalar_enum(ty) {
                    return Err(Error::new(
                        "E0271",
                        e.span,
                        "construct a data enum with a variant and its payload",
                    ));
                }
                if args.len() != 1 {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "enum conversion expects one i32",
                    ));
                }
                let arg = self.expr(&args[0], Some(Type::I32))?;
                Expr {
                    ty,
                    kind: ExprKind::EnumFromInt(Box::new(arg)),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind,A::Name(n) if self.sys_signature(n).is_some()) =>
            {
                let A::Name(name) = &callee.kind else {
                    unreachable!()
                };
                let (op, count, ty) = self.sys_signature(name).unwrap();
                if args.len() != count {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "invalid sys intrinsic argument count",
                    ));
                }
                let mut values = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let expected = if op == SysOp::Arg {
                        Type::I32
                    } else if (op == SysOp::WriteBytes && i == 1) || op == SysOp::DecodeUtf8 {
                        let elem = if name.ends_with("_u8") {
                            Type::U8
                        } else {
                            Type::I32
                        };
                        let array = self.types.dynamic(elem, arg.span)?;
                        self.types.reference(array, false)
                    } else {
                        Type::String
                    };
                    values.push(self.expr(arg, Some(expected))?);
                }
                Expr {
                    ty,
                    kind: ExprKind::SysCall(op, values, self.storage(ty)),
                }
            }
            A::Float(n) => self.float(n, expected.unwrap_or(Type::F64), e.span)?,
            A::String(s) => Expr {
                ty: Type::String,
                kind: ExprKind::String(s.clone()),
            },
            A::Rune(r) => Expr {
                ty: Type::Rune,
                kind: ExprKind::Number(*r as i64),
            },
            A::Record(name, fields) => {
                let ty = self.constructor_type(name)?;
                self.check_record_access(ty, e.span)?;
                let record = self.types.record(ty).ok_or_else(||Error::new("E0240",e.span,"a struct literal requires a struct type; construct enum variants with Type.Variant"))?.clone();
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
                let hint = expected
                    .and_then(|t| self.types.dynamic_elem(t))
                    .or_else(|| context.map(|a| a.elem));
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
                if let Some(ty) = expected.filter(|t| self.types.dynamic_elem(*t).is_some()) {
                    let storage = self.storage(ty).unwrap();
                    return Ok(Expr {
                        ty,
                        kind: ExprKind::Dynamic(checked, storage),
                    });
                }
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
                let base = self.index_receiver(base)?;
                if matches!(base.ty, Type::String | Type::Bytes | Type::Runes) {
                    let arg = self.expr(index, None)?;
                    let (op, ty, arg) = if base.ty == Type::String && arg.ty == Type::Range {
                        (StringOp::Slice, Type::String, arg)
                    } else {
                        let op = if base.ty == Type::Runes {
                            StringOp::RuneAt
                        } else {
                            StringOp::ByteAt
                        };
                        let ty = if base.ty == Type::Runes {
                            Type::Rune
                        } else {
                            Type::I32
                        };
                        if arg.ty != Type::I32 {
                            return Err(mismatch(self.types, index.span, Type::I32, arg.ty));
                        }
                        (op, ty, arg)
                    };
                    let typed = Expr {
                        ty,
                        kind: ExprKind::StringOp(op, vec![base, arg]),
                    };
                    if let Some(t) = expected {
                        if t != ty {
                            return Err(mismatch(self.types, e.span, t, ty));
                        }
                    }
                    return Ok(typed);
                }
                if !matches!(base.ty, Type::Array(_) | Type::Dynamic(_)) {
                    return Err(Error::new(
                        "E0221",
                        e.span,
                        "indexing requires a fixed array",
                    ));
                }
                let item = self
                    .types
                    .dynamic_elem(base.ty)
                    .or_else(|| self.types.array(base.ty).map(|a| a.elem))
                    .unwrap();
                let index = self.expr(index, Some(Type::I32))?;
                Expr {
                    ty: item,
                    kind: ExprKind::Index(Box::new(base), Box::new(index)),
                }
            }
            A::Number(n) => self.number(
                *n,
                expected.filter(|t| t.numeric()).unwrap_or(Type::I32),
                e.span,
            )?,
            A::Bool(b) => Expr {
                ty: Type::Bool,
                kind: ExprKind::Bool(*b),
            },
            A::Name(n) => {
                if let Some(b) = self.lookup(n) {
                    self.check_scratch_value(&b, e.span)?;
                    if let Some((ty, _)) = self.types.referenced(b.ty) {
                        Expr {
                            ty,
                            kind: ExprKind::Deref(Box::new(Expr {
                                ty: b.ty,
                                kind: ExprKind::ReferenceAddress(b.slot),
                            })),
                        }
                    } else {
                        Expr {
                            ty: b.ty,
                            kind: ExprKind::Local(b.slot),
                        }
                    }
                } else if let Some((index, ty)) = self.global(n) {
                    self.check_capability(&self.symbols[self.module].functions[n], e.span)?;
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
                if name == "len"
                    && self.hint(base).is_some_and(|ty| {
                        matches!(ty, Type::String | Type::Bytes | Type::Runes)
                            || self.types.array(ty).is_some()
                            || self.types.dynamic_elem(ty).is_some()
                    })
                {
                    let base = self.index_receiver(base)?;
                    self.length(base, e.span)?
                } else if let Some(ty) = self
                    .hint(base)
                    .filter(|ty| self.types.record(*ty).is_some())
                {
                    self.check_record_access(ty, e.span)?;
                    let record = self.types.record(ty).unwrap();
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
                    expected.filter(|t| t.numeric()).unwrap_or(Type::I32),
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
                if *op == "-" && (!x.ty.numeric() || x.ty.unsigned()) {
                    return Err(Error::new(
                        "E0206",
                        e.span,
                        "unary '-' requires a signed integer or floating value",
                    ));
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
                            .filter(|t| t.numeric())
                            .or_else(|| self.hint(left))
                            .or_else(|| self.hint(right))
                    } else {
                        self.hint(left).or_else(|| self.hint(right))
                    }
                    .or_else(|| {
                        (Self::float_literal(left) || Self::float_literal(right))
                            .then_some(Type::F64)
                    });
                    let a = self.expr(left, hint)?;
                    let b = self.expr(right, Some(a.ty))?;
                    let string_op = a.ty == Type::String
                        && matches!(*op, "+" | "==" | "!=" | "<" | "<=" | ">" | ">=");
                    let rune_op = a.ty == Type::Rune && !arithmetic;
                    let enum_op = self.types.scalar_enum(a.ty) && matches!(*op, "==" | "!=");
                    let float_op = a.ty.float() && *op != "%";
                    if !(a.ty.integer()
                        || float_op
                        || enum_op
                        || string_op
                        || rune_op
                        || matches!(*op, "==" | "!=") && matches!(a.ty, Type::Bool | Type::Ptr))
                    {
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
            A::Call(callee, args) if matches!(&callee.kind, A::Member(base,name) if name == "from_bits" && matches!(&base.kind,A::Name(n) if n=="f32"||n=="f64")) =>
            {
                if args.len() != 1 {
                    return Err(Error::new("E0207", e.span, "from_bits expects one integer"));
                }
                let A::Member(base, _) = &callee.kind else {
                    unreachable!()
                };
                let ty = if matches!(&base.kind,A::Name(n) if n=="f32") {
                    Type::F32
                } else {
                    Type::F64
                };
                let bits = self.expr(
                    &args[0],
                    Some(if ty == Type::F32 {
                        Type::I32
                    } else {
                        Type::I64
                    }),
                )?;
                Expr {
                    ty,
                    kind: ExprKind::FloatOp(FloatOp::FromBits, Box::new(bits)),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind, A::Member(base,name) if self.receiver_type(base).is_some_and(Type::float) && name != "str") =>
            {
                if !args.is_empty() {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "this floating method takes no arguments",
                    ));
                }
                let A::Member(base, name) = &callee.kind else {
                    unreachable!()
                };
                let base = self.expr(base, expected.filter(|ty| ty.float()))?;
                let op = match name.as_str() {
                    "abs" => FloatOp::Abs,
                    "sqrt" => FloatOp::Sqrt,
                    "floor" => FloatOp::Floor,
                    "ceil" => FloatOp::Ceil,
                    "round" => FloatOp::Round,
                    "round_ties_even" => FloatOp::RoundEven,
                    "trunc" => FloatOp::Trunc,
                    "is_nan" => FloatOp::IsNan,
                    "is_finite" => FloatOp::IsFinite,
                    "is_infinite" => FloatOp::IsInfinite,
                    "is_sign_negative" => FloatOp::SignNegative,
                    "to_bits" => FloatOp::ToBits,
                    _ => return Err(Error::new("E0261", e.span, "unknown floating method")),
                };
                let ty = match op {
                    FloatOp::IsNan
                    | FloatOp::IsFinite
                    | FloatOp::IsInfinite
                    | FloatOp::SignNegative => Type::Bool,
                    FloatOp::ToBits => {
                        if base.ty == Type::F32 {
                            Type::I32
                        } else {
                            Type::I64
                        }
                    }
                    _ => base.ty,
                };
                Expr {
                    ty,
                    kind: ExprKind::FloatOp(op, Box::new(base)),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind,A::Name(n) if n=="len") => {
                if args.len() != 1 {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "len expects one string, array or iterator",
                    ));
                }
                let base = self.index_receiver(&args[0])?;
                self.length(base, e.span)?
            }
            A::Call(callee, args) if matches!(&callee.kind,A::Member(base,name) if matches!(name.as_str(),"iter"|"len"|"str"|"bytes"|"runes") && !self.module_alias(base)) =>
            {
                if !args.is_empty() {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "this built-in method takes no arguments",
                    ));
                }
                let A::Member(base, name) = &callee.kind else {
                    unreachable!()
                };
                let base = self.expr(base, None)?;
                if name == "len" {
                    self.length(base, e.span)?
                } else if name == "str"
                    && (base.ty.unsigned()
                        || matches!(
                            base.ty,
                            Type::I32
                                | Type::I64
                                | Type::Bool
                                | Type::Rune
                                | Type::String
                                | Type::F32
                                | Type::F64
                        ))
                {
                    self.needs_float_format |= base.ty.float();
                    Expr {
                        ty: Type::String,
                        kind: ExprKind::StringOp(StringOp::ToString, vec![base]),
                    }
                } else if base.ty == Type::String && matches!(name.as_str(), "bytes" | "runes") {
                    Expr {
                        ty: if name == "bytes" {
                            Type::Bytes
                        } else {
                            Type::Runes
                        },
                        kind: ExprKind::StringView(Box::new(base)),
                    }
                } else if name == "iter" && self.types.array(base.ty).is_some() {
                    let id = match base.ty {
                        Type::Array(i) | Type::Iterator(i) => i,
                        _ => unreachable!(),
                    };
                    Expr {
                        ty: Type::Iterator(id),
                        kind: ExprKind::Iterator(Box::new(base)),
                    }
                } else {
                    return Err(Error::new(
                        "E0250",
                        e.span,
                        "method is not supported by this value",
                    ));
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
                if !value.ty.unsigned()
                    && !matches!(
                        value.ty,
                        Type::I32
                            | Type::I64
                            | Type::Bool
                            | Type::String
                            | Type::Rune
                            | Type::F32
                            | Type::F64
                    )
                {
                    return Err(Error::new("E0206", e.span, "print requires a scalar value"));
                }
                self.needs_float_format |= value.ty.float();
                Expr {
                    ty: Type::Unit,
                    kind: ExprKind::Print(Box::new(value)),
                }
            }
            A::Call(callee, args) if matches!(&callee.kind, A::Name(n) if (n == "rune" || ffi::scalar(n).is_some())) =>
            {
                if args.len() != 1 {
                    return Err(Error::new(
                        "E0207",
                        e.span,
                        "a numeric conversion expects one argument",
                    ));
                }
                let A::Name(name) = &callee.kind else {
                    unreachable!()
                };
                let ty = ffi::scalar(name).unwrap_or(Type::Rune);
                let inner = self.expr(
                    &args[0],
                    self.hint(&args[0])
                        .or(Some(if Self::float_literal(&args[0]) && !ty.float() {
                            Type::F64
                        } else if ty == Type::Rune {
                            Type::I32
                        } else {
                            ty
                        })),
                )?;
                let address_bits = ty == Type::Usize
                    && (inner.ty == Type::Ptr || self.types.is_extern_fn(inner.ty));
                if address_bits {
                    self.require_unsafe(e.span)?;
                }
                if !(address_bits
                    || inner.ty.numeric()
                    || inner.ty == Type::Rune
                    || (ty.integer() && self.types.scalar_enum(inner.ty)))
                    || (ty.float() && inner.ty == Type::Rune)
                    || (ty == Type::Rune && inner.ty.float())
                {
                    return Err(Error::new(
                        "E0206",
                        e.span,
                        "numeric conversion requires a compatible numeric value",
                    ));
                }
                if ty == Type::Rune {
                    Expr {
                        ty,
                        kind: ExprKind::StringOp(StringOp::ToRune, vec![inner]),
                    }
                } else {
                    Expr {
                        ty,
                        kind: ExprKind::Cast(Box::new(inner)),
                    }
                }
            }
            A::Call(callee, args) => {
                let target = self.expr(callee, None)?;
                if matches!(target.ty, Type::UnsafeFunction(_) | Type::ExternFunction(_)) {
                    self.require_unsafe(callee.span)?;
                }
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
                self.check_borrow_aliases(args)?;
                let values = args
                    .iter()
                    .zip(types)
                    .map(|(a, t)| {
                        let value = self.expr(a, Some(t))?;
                        if let Some(storage) = self.storage(value.ty) {
                            Ok(Expr {
                                ty: value.ty,
                                kind: ExprKind::Clone(Box::new(value), storage),
                            })
                        } else {
                            Ok(value)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Expr {
                    ty: ret,
                    kind: ExprKind::Call(Box::new(target), values, self.storage(ret)),
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
    fn length(&self, base: Expr, span: Span) -> Result<Expr, Error> {
        let kind = match base.ty {
            Type::Dynamic(_) => ExprKind::DynamicOp(DynamicOp::Len, Box::new(base), None),
            Type::String | Type::Bytes => ExprKind::StringOp(StringOp::Len, vec![base]),
            Type::Runes => ExprKind::StringOp(StringOp::RuneLen, vec![base]),
            ty => {
                let len = self
                    .types
                    .array(ty)
                    .ok_or_else(|| {
                        Error::new("E0221", span, "len requires a string, array or iterator")
                    })?
                    .len;
                ExprKind::Len(Box::new(base), len)
            }
        };
        Ok(Expr {
            ty: Type::I32,
            kind,
        })
    }
    fn block(&mut self, stmts: &[ast::Stmt]) -> Result<(Vec<Stmt>, bool), Error> {
        let first_owner = self.owned_storage.len();
        self.scopes.push(HashMap::new());
        let result = self.statements(stmts);
        self.scopes.pop();
        result.map(|(mut body, returns)| {
            body.push(Stmt::DropStorage(
                self.owned_storage[first_owner..].to_vec(),
            ));
            (body, returns)
        })
    }
    fn statements(&mut self, stmts: &[ast::Stmt]) -> Result<(Vec<Stmt>, bool), Error> {
        let mut result = Vec::new();
        let mut returns = false;
        for s in stmts {
            let (stmt, ends) = match &s.kind {
                ast::StmtKind::Region(name, capacity, body) => {
                    self.check_region(name, capacity.as_ref(), body, s.span)?
                }
                ast::StmtKind::Match(source, arms) => self.check_match(source, arms, s.span)?,
                ast::StmtKind::Decl {
                    name,
                    ty,
                    mutable,
                    init,
                } => {
                    if let Some(allocation) =
                        self.scratch_declaration(name, *ty, *mutable, init, s.span)?
                    {
                        result.push(allocation);
                        continue;
                    }
                    let ty = ty
                        .map(|ty| resolve_type(self.types, self.symbols, ty, s.span))
                        .transpose()?;
                    let x = self.expr(init, ty)?;
                    if self.types.referenced(x.ty).is_some() {
                        return Err(Error::new(
                            "E0281",
                            s.span,
                            "references are currently restricted to function parameters",
                        ));
                    }
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
                    self.check_scratch_value(&b, s.span)?;
                    if let Some((ty, mutable)) = self.types.referenced(b.ty) {
                        if !mutable {
                            return Err(Error::new(
                                "E0208",
                                s.span,
                                "cannot assign through a shared reference",
                            ));
                        }
                        let value = self.expr(value, Some(ty))?;
                        result.push(Stmt::StorePlace(
                            Expr {
                                ty,
                                kind: ExprKind::ReferenceAddress(b.slot),
                            },
                            value,
                        ));
                        continue;
                    }
                    if !b.mutable {
                        return Err(Error::new("E0208", s.span, format!("'{n}' is immutable; declare it with 'var {n} := ...' to allow assignment")));
                    }
                    let x = self.expr(value, Some(b.ty))?;
                    (self.store_value(&b, x), false)
                }
                ast::StmtKind::AssignIndex(target, value) => {
                    self.check_index_assignment(target, value)?;
                    let (ty, address) = self.place(target, true)?;
                    let value = self.expr(value, Some(ty))?;
                    (Stmt::StorePlace(address, value), false)
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
                    if let ast::ExprKind::Call(callee, args) = &e.kind {
                        if let ast::ExprKind::Name(name) = &callee.kind {
                            if !self.bound_name(name)
                                && name.strip_prefix("store_").and_then(ffi::scalar).is_some()
                            {
                                result.push(self.raw_store(name, args, s.span)?);
                                continue;
                            }
                        }
                    }
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
                    self.loop_owned_starts.push(self.owned_storage.len());
                    let (body, _) = self.block(body)?;
                    self.loop_owned_starts.pop();
                    self.loops -= 1;
                    (Stmt::While(cond, body), false)
                }
                ast::StmtKind::For(name, source, body) => {
                    let source = self.expr(source, None)?;
                    let item = if source.ty == Type::Range {
                        Type::I32
                    } else if source.ty == Type::Runes {
                        Type::Rune
                    } else if source.ty == Type::Bytes {
                        Type::I32
                    } else if let Some(elem) = self.types.dynamic_elem(source.ty) {
                        elem
                    } else {
                        self.types
                            .array(source.ty)
                            .ok_or_else(|| {
                                Error::new(
                                    "E0222",
                                    s.span,
                                    "for requires a range, array, array iterator, string.bytes() or string.runes()",
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
                    self.loop_owned_starts.push(self.owned_storage.len());
                    let (body, _) = self.block(body)?;
                    self.loop_owned_starts.pop();
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
                ast::StmtKind::Unsafe(b) => {
                    let was_unsafe = self.in_unsafe;
                    self.in_unsafe = true;
                    let (b, r) = self.block(b)?;
                    self.in_unsafe = was_unsafe;
                    (Stmt::Block(b), r)
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
                    let first_owner = *self.loop_owned_starts.last().unwrap();
                    result.push(Stmt::DropStorage(
                        self.owned_storage[first_owner..].to_vec(),
                    ));
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
            filesystem: program.filesystem,
            functions: program.functions,
            imports: Vec::new(),
            records: program.records,
            enums: program.enums,
            standard_sys: false,
        }],
        program.types,
        mode,
    )
}

pub(crate) fn check_modules(
    modules: Vec<Module>,
    types: Types,
    mode: Mode,
) -> Result<Program, Error> {
    check_reporting(modules, types, mode, None)
}

pub(crate) fn check_reporting(
    mut modules: Vec<Module>,
    mut types: Types,
    mode: Mode,
    mut reports: Option<&mut Vec<BindingInfo>>,
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
        symbols[module].filesystem = input.filesystem;
        symbols[module].standard_sys = input.standard_sys;
        let mut names = std::collections::HashSet::new();
        for e in &input.enums {
            if reserved(&e.name) {
                return Err(Error::new("E0200", e.span, "reserved enum name").in_file(module));
            }
            if !names.insert(e.name.clone()) {
                return Err(Error::new("E0203", e.span, "duplicate enum name").in_file(module));
            }
            let ty = types.declare_enum(
                module,
                e.name.clone(),
                e.variants.iter().any(|v| !v.fields.is_empty()),
            );
            symbols[module]
                .records
                .insert(e.name.clone(), (ty, e.public));
        }
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
            let ty =
                types.declare_record(module, record.name.clone(), record.c_layout, record.opaque);
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
            let mut seen = std::collections::HashSet::new();
            for field in &record.fields {
                if !seen.insert(field.name.clone()) {
                    return Err(
                        Error::new("E0240", field.span, "duplicate struct field").in_file(module)
                    );
                }
                let ty = resolve_unvalidated(&mut types, &symbols, field.ty, field.span)
                    .map_err(|e| e.in_file(module))?;
                fields.push(ast::RecordField {
                    name: field.name.clone(),
                    ty,
                    offset: 0,
                });
            }
            types.define_record(ty, fields, 0);
        }
    }
    let mut index = 0;
    enums::define_enums(&modules, &symbols, &mut types)?;
    // A callback signature in an earlier record can use a C aggregate declared
    // later. Its ABI class is known once every field has been laid out.
    for (module, input) in modules.iter().enumerate() {
        for record in &input.records {
            let ty = symbols[module].records[&record.name].0;
            for (field, parsed) in types.record(ty).unwrap().fields.iter().zip(&record.fields) {
                ffi::validate_type(&types, field.ty, parsed.span).map_err(|e| e.in_file(module))?;
            }
        }
        for e in &input.enums {
            let ty = symbols[module].records[&e.name].0;
            for (variant, parsed) in types
                .enumeration(ty)
                .unwrap()
                .variants
                .iter()
                .zip(&e.variants)
            {
                for field in &variant.fields {
                    ffi::validate_type(&types, field.ty, parsed.span)
                        .map_err(|e| e.in_file(module))?;
                }
            }
        }
    }
    for (module, input) in modules.iter_mut().enumerate() {
        let mut test_names = std::collections::HashSet::new();
        for f in &mut input.functions {
            for p in &mut f.params {
                p.ty = resolve_type(&mut types, &symbols, p.ty, p.span)
                    .map_err(|e| e.in_file(module))?;
            }
            f.ret =
                resolve_type(&mut types, &symbols, f.ret, f.span).map_err(|e| e.in_file(module))?;
            if types.referenced(f.ret).is_some() {
                return Err(
                    Error::new("E0281", f.span, "returning references is not supported")
                        .in_file(module),
                );
            }
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
            let params: Vec<Type> = f.params.iter().map(|p| p.ty).collect();
            if f.external.is_some() {
                ffi::signature(&types, &params, f.ret, f.span).map_err(|e| e.in_file(module))?;
            }
            let Type::Function(index_ty) = types.function(params, f.ret) else {
                unreachable!()
            };
            let ty = if f.external.is_some() {
                Type::ExternFunction(index_ty)
            } else if f.unsafe_fn {
                Type::UnsafeFunction(index_ty)
            } else {
                Type::Function(index_ty)
            };
            symbols[module].functions.insert(
                f.name.clone(),
                Global {
                    requires_filesystem: f.requires_filesystem,
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
        if entry.requires_filesystem
            || !matches!(entry.ty, Type::Function(_))
            || !sig.params.is_empty()
            || !matches!(sig.ret, Type::I32 | Type::Unit)
        {
            return Err(Error::new(
                "E0212",
                modules[0].functions[entry.index].span,
                "main must take no arguments and return i32 or ()",
            ));
        }
    }
    let main = entry.map(|e| e.index);
    let sys_types = symbols.iter().find(|s| s.standard_sys).map(|s| SysTypes {
        status: s.records["Status"].0,
        text: s.records["TextResult"].0,
        metadata: s.records["Metadata"].0,
    });
    let mut checked = Vec::new();
    let mut needs_float_format = false;
    for (module, input) in modules.into_iter().enumerate() {
        for f in input.functions {
            let function = (|| {
                let mut c = Checker {
                    reports: reports.as_deref_mut(),
                    module,
                    symbols: &symbols,
                    types: &mut types,
                    scopes: vec![HashMap::new()],
                    regions: Default::default(),
                    slots: 0,
                    owned_storage: Vec::new(),
                    ret: f.ret,
                    loops: 0,
                    loop_owned_starts: Vec::new(),
                    testing: f.test.is_some(),
                    in_unsafe: f.unsafe_fn,
                    needs_float_format: false,
                };
                let mut param_slots = Vec::new();
                let mut param_storage = Vec::new();
                let return_slot = c.types.storage_bytes(f.ret).map(|_| c.slot());
                for p in &f.params {
                    param_slots.push(c.bind(&p.name, p.ty, false, p.span)?);
                    param_storage.push(c.lookup(&p.name).unwrap().storage);
                }
                let (body, returns) = c.statements(&f.body)?;
                needs_float_format |= c.needs_float_format;
                if f.external.is_none() && f.ret != Type::Unit && !returns {
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
                    ret: f.ret,
                    external: f.external,
                    name: format!("m{module}.{}", f.name),
                    params: f.params.iter().map(|p| p.ty).collect(),
                    slots: c.slots,
                    body,
                    param_slots,
                    param_storage,
                    return_slot,
                    owned_storage: c.owned_storage,
                    uses_float_format: c.needs_float_format,
                })
            })()
            .map_err(|e: Error| e.in_file(module))?;
            checked.push(function);
        }
    }
    let float_formatter = if needs_float_format {
        let index = checked.len();
        checked.push(float_formatter_function(&mut types)?);
        Some(index)
    } else {
        None
    };
    Ok(Program {
        types,
        functions: checked,
        main,
        tests,
        float_formatter,
        sys_types,
    })
}

fn float_formatter_function(types: &mut Types) -> Result<Function, Error> {
    let source = include_str!("../runtime/float.ddr");
    let mut build = || -> Result<Function, Error> {
        let ast = ast::parse_module_at(source, types, 0)?;
        let [f] = ast.functions.as_slice() else {
            return Err(Error::new(
                "E0999",
                Span { start: 0, end: 0 },
                "float runtime must define one formatter",
            ));
        };
        if f.params.iter().map(|p| p.ty).collect::<Vec<_>>() != [Type::I64, Type::Bool]
            || f.ret != Type::String
        {
            return Err(Error::new(
                "E0999",
                f.span,
                "float formatter must have signature fn(i64, bool) -> string",
            ));
        }
        let symbols = [Symbols::default()];
        let mut c = Checker {
            reports: None,
            module: 0,
            symbols: &symbols,
            types,
            scopes: vec![HashMap::new()],
            regions: Default::default(),
            slots: 0,
            owned_storage: Vec::new(),
            ret: Type::String,
            loops: 0,
            loop_owned_starts: Vec::new(),
            testing: false,
            in_unsafe: false,
            needs_float_format: false,
        };
        let mut param_slots = Vec::new();
        for p in &f.params {
            param_slots.push(c.bind(&p.name, p.ty, false, p.span)?);
        }
        let (body, returns) = c.statements(&f.body)?;
        if !returns {
            return Err(Error::new(
                "E0999",
                f.span,
                "float formatter must return on every path",
            ));
        }
        Ok(Function {
            ret: Type::String,
            external: None,
            name: "<dunder:float-format>".into(),
            params: vec![Type::I64, Type::Bool],
            slots: c.slots,
            body,
            param_slots,
            param_storage: vec![None, None],
            return_slot: None,
            owned_storage: c.owned_storage,
            uses_float_format: false,
        })
    };
    build().map_err(|e| {
        Error::new(
            "E0999",
            Span { start: 0, end: 0 },
            e.render("<dunder:float-format>", source),
        )
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
            format!("type '{name}' is private; export it with 'pub struct' or 'pub enum'"),
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
    let ty = resolve_unvalidated(types, symbols, ty, span)?;
    ffi::validate_type(types, ty, span)?;
    Ok(ty)
}

fn resolve_unvalidated(
    types: &mut Types,
    symbols: &[Symbols],
    ty: Type,
    span: Span,
) -> Result<Type, Error> {
    if let Some(elem) = types.dynamic_elem(ty) {
        let elem = resolve_unvalidated(types, symbols, elem, span)?;
        return types.dynamic(elem, span);
    }
    if let Some((inner, mutable)) = types.referenced(ty) {
        let inner = resolve_unvalidated(types, symbols, inner, span)?;
        if matches!(inner, Type::Unit | Type::Reference(_)) {
            return Err(Error::new("E0281", span, "reference a value type"));
        }
        return Ok(types.reference(inner, mutable));
    }
    if let Some(named) = types.named_ref(ty).cloned() {
        return lookup_type(symbols, named.module, &named.name, span);
    }
    if matches!(ty, Type::Array(_)) {
        let array = types.array(ty).unwrap();
        let elem = resolve_unvalidated(types, symbols, array.elem, span)?;
        return types.array_type(elem, array.len, span);
    }
    if let Some(signature) = types.signature(ty).cloned() {
        let params = signature
            .params
            .into_iter()
            .map(|t| resolve_unvalidated(types, symbols, t, span))
            .collect::<Result<Vec<_>, _>>()?;
        let ret = resolve_unvalidated(types, symbols, signature.ret, span)?;
        if types.is_extern_fn(ty) {
            let Type::Function(i) = types.function(params, ret) else {
                unreachable!()
            };
            return Ok(Type::ExternFunction(i));
        }
        return Ok(types.function(params, ret));
    }
    Ok(ty)
}
