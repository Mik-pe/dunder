use crate::{Error, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I32,
    I64,
    Bool,
    String,
    Rune,
    Bytes,
    Runes,
    Unit,
    Function(usize),
    Array(usize),
    Iterator(usize),
    Range,
    Named(usize),
    Record(usize),
}
impl Type {
    pub fn integer(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::String => "string",
            Self::Rune => "rune",
            Self::Bytes => "string.bytes",
            Self::Runes => "string.runes",
            Self::Unit => "()",
            Self::Function(_) => "fn",
            Self::Array(_) => "array",
            Self::Iterator(_) => "iterator",
            Self::Range => "range",
            Self::Named(_) => "type",
            Self::Record(_) => "struct",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionType {
    pub params: Vec<Type>,
    pub ret: Type,
}

#[derive(Default, Debug)]
pub struct Types {
    functions: Vec<FunctionType>,
    arrays: Vec<ArrayType>,
    names: Vec<NamedType>,
    records: Vec<RecordType>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedType {
    pub module: usize,
    pub name: String,
}
#[derive(Clone, Debug)]
pub struct RecordField {
    pub name: String,
    pub ty: Type,
    pub offset: usize,
}
#[derive(Clone, Debug)]
pub struct RecordType {
    pub name: String,
    pub module: usize,
    pub fields: Vec<RecordField>,
    pub bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArrayType {
    pub elem: Type,
    pub len: usize,
}
impl ArrayType {
    pub fn elem_size(self) -> usize {
        match self.elem {
            Type::Bool => 1,
            Type::I32 | Type::Rune => 4,
            _ => 8,
        }
    }
    pub fn bytes(self) -> usize {
        self.elem_size() * self.len
    }
}
impl Types {
    pub fn named(&mut self, module: usize, name: String) -> Type {
        let item = NamedType { module, name };
        let index = self
            .names
            .iter()
            .position(|n| *n == item)
            .unwrap_or_else(|| {
                self.names.push(item);
                self.names.len() - 1
            });
        Type::Named(index)
    }
    pub fn named_ref(&self, ty: Type) -> Option<&NamedType> {
        if let Type::Named(i) = ty {
            self.names.get(i)
        } else {
            None
        }
    }
    pub fn declare_record(&mut self, module: usize, name: String) -> Type {
        let index = self.records.len();
        self.records.push(RecordType {
            name,
            module,
            fields: Vec::new(),
            bytes: 1,
        });
        Type::Record(index)
    }
    pub fn record(&self, ty: Type) -> Option<&RecordType> {
        if let Type::Record(i) = ty {
            self.records.get(i)
        } else {
            None
        }
    }
    pub fn define_record(&mut self, ty: Type, fields: Vec<RecordField>, bytes: usize) {
        if let Type::Record(i) = ty {
            self.records[i].fields = fields;
            self.records[i].bytes = bytes;
        }
    }
    pub fn storage_bytes(&self, ty: Type) -> Option<usize> {
        self.array(ty)
            .map(|a| a.bytes())
            .or_else(|| self.record(ty).map(|r| r.bytes))
    }
    pub fn array_type(&mut self, elem: Type, len: usize, span: Span) -> Result<Type, Error> {
        if !matches!(
            elem,
            Type::I32 | Type::I64 | Type::Bool | Type::String | Type::Rune
        ) {
            return Err(Error::new(
                "E0220",
                span,
                "array elements must be i32, i64, bool, string or rune in this version",
            ));
        }
        let array = ArrayType { elem, len };
        if len > 65536 || array.bytes() > 65536 {
            return Err(Error::new(
                "E0220",
                span,
                "a fixed array may occupy at most 65536 bytes in this version",
            ));
        }
        let index = self
            .arrays
            .iter()
            .position(|a| *a == array)
            .unwrap_or_else(|| {
                self.arrays.push(array);
                self.arrays.len() - 1
            });
        Ok(Type::Array(index))
    }
    pub fn array(&self, ty: Type) -> Option<ArrayType> {
        match ty {
            Type::Array(i) | Type::Iterator(i) => self.arrays.get(i).copied(),
            _ => None,
        }
    }
    pub fn function(&mut self, params: Vec<Type>, ret: Type) -> Type {
        let signature = FunctionType { params, ret };
        let index = self
            .functions
            .iter()
            .position(|f| *f == signature)
            .unwrap_or_else(|| {
                self.functions.push(signature);
                self.functions.len() - 1
            });
        Type::Function(index)
    }
    pub fn signature(&self, ty: Type) -> Option<&FunctionType> {
        if let Type::Function(i) = ty {
            self.functions.get(i)
        } else {
            None
        }
    }
    pub fn name(&self, ty: Type) -> String {
        if let Some(n) = self.named_ref(ty) {
            return n.name.clone();
        }
        if let Some(r) = self.record(ty) {
            return r.name.clone();
        }
        if let Some(a) = self.array(ty) {
            let name = format!("[{}; {}]", self.name(a.elem), a.len);
            return if matches!(ty, Type::Iterator(_)) {
                format!("iter<{name}>")
            } else {
                name
            };
        }
        match self.signature(ty) {
            Some(f) => format!(
                "fn({}) -> {}",
                f.params
                    .iter()
                    .map(|t| self.name(*t))
                    .collect::<Vec<_>>()
                    .join(", "),
                self.name(f.ret)
            ),
            None => ty.name().into(),
        }
    }
}

pub struct Program {
    pub functions: Vec<Function>,
    pub types: Types,
    pub imports: Vec<Import>,
    pub records: Vec<Record>,
}

pub struct Module {
    pub functions: Vec<Function>,
    pub imports: Vec<Import>,
    pub records: Vec<Record>,
}
#[derive(Debug)]
pub struct Record {
    pub name: String,
    pub public: bool,
    pub fields: Vec<Field>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug)]
pub struct Import {
    pub path: String,
    pub alias: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Name(String),
    Number(i128),
    String(String),
    Rune(char),
    Left,
    Right,
    Square,
    EndSquare,
    Open,
    Close,
    Colon,
    Define,
    Equal,
    Comma,
    Dot,
    Range,
    Semi,
    At,
    Newline,
    Arrow,
    Op(&'static str),
    Eof,
}
#[derive(Clone, Debug)]
struct Lexeme {
    token: Token,
    span: Span,
}

fn lex(source: &str) -> Result<Vec<Lexeme>, Error> {
    let mut out = Vec::new();
    let chars: Vec<_> = source.char_indices().collect();
    let mut i = 0;
    let mut parens = 0usize;
    while i < chars.len() {
        let (start, c) = chars[i];
        let next = chars.get(i + 1).map(|x| x.1);
        if c == '\n' && parens == 0 {
            out.push(Lexeme {
                token: Token::Newline,
                span: Span {
                    start,
                    end: start + 1,
                },
            });
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if matches!(c, '"' | '\'' | '`') {
            i += 1;
            let mut bytes = Vec::new();
            let mut closed = false;
            while i < chars.len() {
                let ch = chars[i].1;
                if ch == c {
                    i += 1;
                    closed = true;
                    break;
                }
                if ch == '\n' || ch == '\r' {
                    break;
                }
                if ch == '\\' {
                    i += 1;
                    match chars.get(i).map(|x| x.1) {
                        Some(ch @ ('\\' | '"' | '\'' | '`')) => bytes.push(ch as u8),
                        Some('n') => bytes.push(b'\n'),
                        Some('r') => bytes.push(b'\r'),
                        Some('t') => bytes.push(b'\t'),
                        Some('0') => bytes.push(0),
                        Some('x') => {
                            let mut byte = 0;
                            for _ in 0..2 {
                                i += 1;
                                let digit = chars
                                    .get(i)
                                    .and_then(|(_, ch)| ch.to_digit(16))
                                    .ok_or_else(|| {
                                        Error::new(
                                            "E0005",
                                            Span {
                                                start,
                                                end: chars.get(i).map_or(source.len(), |x| x.0),
                                            },
                                            "\\x requires exactly two hex digits",
                                        )
                                    })?;
                                byte = byte * 16 + digit;
                            }
                            bytes.push(byte as u8);
                        }
                        Some('u') => {
                            let braced = chars.get(i + 1).is_some_and(|x| x.1 == '{');
                            if braced {
                                i += 1;
                            }
                            let mut scalar = 0u32;
                            let mut count = 0;
                            loop {
                                if braced && chars.get(i + 1).is_some_and(|x| x.1 == '}') {
                                    i += 1;
                                    break;
                                }
                                if !braced && count == 4 {
                                    break;
                                }
                                i += 1;
                                let digit = chars
                                    .get(i)
                                    .and_then(|(_, ch)| ch.to_digit(16))
                                    .filter(|_| count < 6)
                                    .ok_or_else(|| {
                                        Error::new(
                                            "E0005",
                                            Span {
                                                start,
                                                end: chars.get(i).map_or(source.len(), |x| x.0),
                                            },
                                            "expected a Unicode escape: \\uXXXX or \\u{X...}",
                                        )
                                    })?;
                                scalar = scalar * 16 + digit;
                                count += 1;
                            }
                            let scalar =
                                char::from_u32(scalar)
                                    .filter(|_| count > 0)
                                    .ok_or_else(|| {
                                        Error::new(
                                            "E0005",
                                            Span {
                                                start,
                                                end: chars.get(i).map_or(source.len(), |x| x.0),
                                            },
                                            "escape must name a Unicode scalar value",
                                        )
                                    })?;
                            bytes.extend_from_slice(scalar.encode_utf8(&mut [0; 4]).as_bytes());
                        }
                        _ => {
                            return Err(Error::new(
                                "E0004",
                                Span {
                                    start,
                                    end: chars.get(i).map_or(source.len(), |x| x.0),
                                },
                                "unknown string escape",
                            ))
                        }
                    }
                } else {
                    bytes.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                }
                i += 1;
            }
            let end = chars.get(i).map_or(source.len(), |x| x.0);
            if !closed {
                return Err(Error::new(
                    "E0004",
                    Span { start, end },
                    "unterminated quoted string",
                ));
            }
            let value = String::from_utf8(bytes).map_err(|_| {
                Error::new(
                    "E0005",
                    Span { start, end },
                    "string literal must contain valid UTF-8",
                )
            })?;
            let token = if c == '`' {
                let mut runes = value.chars();
                let rune = runes
                    .next()
                    .filter(|_| runes.next().is_none())
                    .ok_or_else(|| {
                        Error::new(
                            "E0005",
                            Span { start, end },
                            "a rune literal must contain exactly one Unicode scalar value",
                        )
                    })?;
                Token::Rune(rune)
            } else {
                Token::String(value)
            };
            out.push(Lexeme {
                token,
                span: Span { start, end },
            });
            continue;
        }
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                let pair = (chars[i].1, chars.get(i + 1).map(|x| x.1));
                if pair == ('/', Some('*')) {
                    depth += 1;
                    i += 2;
                } else if pair == ('*', Some('/')) {
                    depth -= 1;
                    i += 2;
                } else {
                    if chars[i].1 == '\n' && parens == 0 {
                        out.push(Lexeme {
                            token: Token::Newline,
                            span: Span {
                                start: chars[i].0,
                                end: chars[i].0 + 1,
                            },
                        });
                    }
                    i += 1;
                }
            }
            if depth != 0 {
                return Err(Error::new(
                    "E0001",
                    Span {
                        start,
                        end: source.len(),
                    },
                    "unterminated block comment",
                ));
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            i += 1;
            while i < chars.len() && (chars[i].1.is_alphanumeric() || chars[i].1 == '_') {
                i += 1;
            }
            let end = chars.get(i).map_or(source.len(), |x| x.0);
            out.push(Lexeme {
                token: Token::Name(source[start..end].into()),
                span: Span { start, end },
            });
            continue;
        }
        if c.is_ascii_digit() {
            i += 1;
            while i < chars.len() && (chars[i].1.is_ascii_digit() || chars[i].1 == '_') {
                i += 1;
            }
            let end = chars.get(i).map_or(source.len(), |x| x.0);
            let n = source[start..end].replace('_', "").parse().map_err(|_| {
                Error::new("E0002", Span { start, end }, "integer literal is too large")
            })?;
            out.push(Lexeme {
                token: Token::Number(n),
                span: Span { start, end },
            });
            continue;
        }
        let pair = match (c, next) {
            (':', Some('=')) => Some(Token::Define),
            ('-', Some('>')) => Some(Token::Arrow),
            ('=', Some('=')) => Some(Token::Op("==")),
            ('!', Some('=')) => Some(Token::Op("!=")),
            ('<', Some('=')) => Some(Token::Op("<=")),
            ('>', Some('=')) => Some(Token::Op(">=")),
            ('&', Some('&')) => Some(Token::Op("&&")),
            ('|', Some('|')) => Some(Token::Op("||")),
            ('.', Some('.')) => Some(Token::Range),
            _ => None,
        };
        let (token, len) = if let Some(t) = pair {
            (t, 2)
        } else {
            (
                match c {
                    '(' => {
                        parens += 1;
                        Token::Left
                    }
                    ')' => {
                        parens = parens.saturating_sub(1);
                        Token::Right
                    }
                    '[' => {
                        parens += 1;
                        Token::Square
                    }
                    ']' => {
                        parens = parens.saturating_sub(1);
                        Token::EndSquare
                    }
                    '{' => Token::Open,
                    '}' => Token::Close,
                    ':' => Token::Colon,
                    '=' => Token::Equal,
                    ',' => Token::Comma,
                    '.' => Token::Dot,
                    ';' => Token::Semi,
                    '@' => Token::At,
                    '+' => Token::Op("+"),
                    '-' => Token::Op("-"),
                    '*' => Token::Op("*"),
                    '/' => Token::Op("/"),
                    '%' => Token::Op("%"),
                    '<' => Token::Op("<"),
                    '>' => Token::Op(">"),
                    '!' => Token::Op("!"),
                    _ => {
                        return Err(Error::new(
                            "E0003",
                            Span {
                                start,
                                end: start + c.len_utf8(),
                            },
                            format!("unexpected character {c:?}"),
                        ))
                    }
                },
                1,
            )
        };
        i += len;
        let end = chars.get(i).map_or(source.len(), |x| x.0);
        out.push(Lexeme {
            token,
            span: Span { start, end },
        });
    }
    out.push(Lexeme {
        token: Token::Eof,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
    });
    Ok(out)
}

#[derive(Debug)]
pub struct Function {
    pub public: bool,
    pub test: Option<String>,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    pub body: Vec<Stmt>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
#[derive(Debug)]
pub enum StmtKind {
    Decl {
        name: String,
        ty: Option<Type>,
        mutable: bool,
        init: Expr,
    },
    Assign(String, Expr),
    AssignIndex(Expr, Expr),
    Return(Option<Expr>),
    Expr(Expr),
    Tail(Expr),
    Case(Vec<Expr>, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Block(Vec<Stmt>),
    Break,
    Continue,
}
#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Debug)]
pub enum ExprKind {
    Number(i128),
    Bool(bool),
    String(String),
    Rune(char),
    Name(String),
    TestTarget(String),
    Member(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Repeat(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Record(Box<Expr>, Vec<(String, Expr, Span)>),
}

struct Parser<'a> {
    tokens: Vec<Lexeme>,
    pos: usize,
    depth: usize,
    types: &'a mut Types,
    cases: bool,
    module: usize,
    record_literals: bool,
}
impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].token
    }
    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }
    fn take(&mut self) -> Lexeme {
        let t = self.tokens[self.pos].clone();
        if t.token != Token::Eof {
            self.pos += 1;
        }
        t
    }
    fn is_word(&self, word: &str) -> bool {
        matches!(self.peek(), Token::Name(n) if n == word)
    }
    fn accept(&mut self, t: Token) -> bool {
        if *self.peek() == t {
            self.take();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Token, description: &str) -> Result<(), Error> {
        if self.accept(t) {
            Ok(())
        } else {
            Err(Error::new(
                "E0100",
                self.span(),
                format!("expected {description}"),
            ))
        }
    }
    fn name(&mut self) -> Result<(String, Span), Error> {
        let t = self.take();
        if let Token::Name(n) = t.token {
            Ok((n, t.span))
        } else {
            Err(Error::new("E0101", t.span, "expected a name"))
        }
    }
    fn separators(&mut self) {
        while matches!(self.peek(), Token::Newline | Token::Semi) {
            self.take();
        }
    }
    fn newlines(&mut self) {
        while self.accept(Token::Newline) {}
    }
    fn end_stmt(&mut self) -> Result<(), Error> {
        if matches!(self.peek(), Token::Newline | Token::Semi) {
            self.separators();
            Ok(())
        } else if *self.peek() == Token::Close {
            Ok(())
        } else {
            Err(Error::new(
                "E0102",
                self.span(),
                "expected a newline, ';' or '}' after statement",
            ))
        }
    }
    fn ty(&mut self) -> Result<Type, Error> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(Error::new("E0104", self.span(), "type nesting is too deep"));
        }
        let result = self.type_inner();
        self.depth -= 1;
        result
    }
    fn type_inner(&mut self) -> Result<Type, Error> {
        if self.accept(Token::Square) {
            let span = self.span();
            let elem = self.ty()?;
            self.expect(Token::Semi, "';' before array length")?;
            let len = self.array_len()?;
            self.expect(Token::EndSquare, "']'")?;
            return self.types.array_type(elem, len, span);
        }
        if self.is_word("fn") {
            self.take();
            self.expect(Token::Left, "'(' after fn")?;
            let mut params = Vec::new();
            while *self.peek() != Token::Right {
                let span = self.span();
                let ty = self.ty()?;
                if ty == Type::Unit {
                    return Err(Error::new(
                        "E0202",
                        span,
                        "a function parameter must have a value type",
                    ));
                }
                params.push(ty);
                if !self.accept(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::Right, "')'")?;
            let ret = if self.accept(Token::Arrow) {
                self.ty()?
            } else {
                Type::Unit
            };
            return Ok(self.types.function(params, ret));
        }
        if self.accept(Token::Left) {
            self.expect(Token::Right, "')'")?;
            return Ok(Type::Unit);
        }
        let (n, _) = self.name()?;
        match n.as_str() {
            "i32" => Ok(Type::I32),
            "i64" => Ok(Type::I64),
            "bool" => Ok(Type::Bool),
            "string" => Ok(Type::String),
            "rune" => Ok(Type::Rune),
            "range" => Ok(Type::Range),
            _ => {
                let mut name = n;
                while self.accept(Token::Dot) {
                    name.push('.');
                    name.push_str(&self.name()?.0);
                }
                Ok(self.types.named(self.module, name))
            }
        }
    }
    fn record_decl(&mut self, public: bool) -> Result<Record, Error> {
        let span = self.take().span;
        let (name, _) = self.name()?;
        self.newlines();
        self.expect(Token::Open, "'{'")?;
        let mut fields = Vec::new();
        self.newlines();
        while *self.peek() != Token::Close {
            let (name, span) = self.name()?;
            self.expect(Token::Colon, "':'")?;
            let ty = self.ty()?;
            fields.push(Field { name, ty, span });
            if self.accept(Token::Comma) || self.accept(Token::Newline) {
                self.newlines();
            } else {
                break;
            }
        }
        self.expect(Token::Close, "'}'")?;
        Ok(Record {
            name,
            public,
            fields,
            span,
        })
    }
    fn header_expr(&mut self) -> Result<Expr, Error> {
        let saved = self.record_literals;
        self.record_literals = false;
        let result = self.expr(0);
        self.record_literals = saved;
        result
    }
    fn array_len(&mut self) -> Result<usize, Error> {
        let t = self.take();
        match t.token {
            Token::Number(n) => {
                usize::try_from(n).map_err(|_| Error::new("E0220", t.span, "invalid array length"))
            }
            _ => Err(Error::new(
                "E0220",
                t.span,
                "array length must be a nonnegative integer literal",
            )),
        }
    }
    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        self.newlines();
        self.expect(Token::Open, "'{'")?;
        self.depth += 1;
        if self.depth > 128 {
            return Err(Error::new("E0104", self.span(), "nesting is too deep"));
        }
        let mut stmts = Vec::new();
        self.separators();
        while *self.peek() != Token::Close {
            if *self.peek() == Token::Eof {
                return Err(Error::new("E0100", self.span(), "expected '}'"));
            }
            stmts.push(self.stmt()?);
            self.separators();
        }
        self.take();
        self.depth -= 1;
        Ok(stmts)
    }
    fn import(&mut self) -> Result<Import, Error> {
        let span = self.take().span;
        let token = self.take();
        let path = match token.token {
            Token::Name(path) | Token::String(path) => path,
            _ => {
                return Err(Error::new(
                    "E0107",
                    token.span,
                    "expected a module name or relative path, for example import math",
                ))
            }
        };
        let alias = if self.is_word("as") {
            self.take();
            self.name()?.0
        } else {
            std::path::Path::new(&path)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .into()
        };
        if *self.peek() != Token::Eof {
            self.end_stmt()?;
        }
        Ok(Import { path, alias, span })
    }
    fn function(&mut self, public: bool) -> Result<Function, Error> {
        let (keyword, span) = self.name()?;
        if keyword != "fn" {
            return Err(Error::new(
                "E0100",
                span,
                "expected 'fn' to declare a function",
            ));
        }
        let (name, _) = self.name()?;
        self.expect(Token::Left, "'('")?;
        let mut params = Vec::new();
        while *self.peek() != Token::Right {
            let (name, span) = self.name()?;
            self.expect(Token::Colon, "':' and a parameter type")?;
            params.push(Param {
                name,
                span,
                ty: self.ty()?,
            });
            if !self.accept(Token::Comma) {
                break;
            }
        }
        self.expect(Token::Right, "')'")?;
        let ret = if self.accept(Token::Arrow) {
            self.ty()?
        } else {
            Type::Unit
        };
        let mut body = self.block()?;
        if ret != Type::Unit {
            implicit_return(&mut body);
        }
        Ok(Function {
            public,
            test: None,
            name,
            params,
            ret,
            body,
            span,
        })
    }
    fn case_start(&self) -> bool {
        if *self.peek() != Token::Left {
            return false;
        }
        let mut depth = 0;
        for i in self.pos..self.tokens.len() {
            match self.tokens[i].token {
                Token::Left => depth += 1,
                Token::Right => {
                    depth -= 1;
                    if depth == 0 {
                        return self.tokens[i + 1..]
                            .iter()
                            .find(|t| t.token != Token::Newline)
                            .is_some_and(|t| t.token == Token::Arrow);
                    }
                }
                Token::Eof => break,
                _ => {}
            }
        }
        false
    }
    fn tail_position(&self) -> bool {
        self.tokens[self.pos..]
            .iter()
            .find(|t| t.token != Token::Newline)
            .is_some_and(|t| t.token == Token::Close)
    }
    fn stmt(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        let kind = if self.cases && self.case_start() {
            self.take();
            let mut args = Vec::new();
            while *self.peek() != Token::Right {
                args.push(self.expr(0)?);
                if !self.accept(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::Right, "')'")?;
            self.newlines();
            self.expect(Token::Arrow, "'->'")?;
            let expected = self.expr(0)?;
            self.end_stmt()?;
            StmtKind::Case(args, expected)
        } else if self.is_word("if") {
            self.take();
            let condition = self.header_expr()?;
            let yes = self.block()?;
            self.newlines();
            let no = if self.is_word("else") {
                self.take();
                if self.is_word("if") {
                    vec![self.stmt()?]
                } else {
                    self.block()?
                }
            } else {
                Vec::new()
            };
            StmtKind::If(condition, yes, no)
        } else if self.is_word("for") {
            self.take();
            let (name, _) = self.name()?;
            if !self.is_word("in") {
                return Err(Error::new(
                    "E0100",
                    self.span(),
                    "expected 'in' after loop variable",
                ));
            }
            self.take();
            let values = self.header_expr()?;
            StmtKind::For(name, values, self.block()?)
        } else if self.is_word("while") {
            self.take();
            let condition = self.header_expr()?;
            StmtKind::While(condition, self.block()?)
        } else if *self.peek() == Token::Open {
            StmtKind::Block(self.block()?)
        } else if self.is_word("return") {
            self.take();
            let value = if matches!(self.peek(), Token::Newline | Token::Semi | Token::Close) {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.end_stmt()?;
            StmtKind::Return(value)
        } else if self.is_word("break") || self.is_word("continue") {
            let is_break = self.is_word("break");
            self.take();
            self.end_stmt()?;
            if is_break {
                StmtKind::Break
            } else {
                StmtKind::Continue
            }
        } else {
            let mutable = self.is_word("var");
            if mutable {
                self.take();
            }
            let decl = matches!(self.peek(), Token::Name(_))
                && matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::Define | Token::Colon)
                );
            if decl {
                let (name, _) = self.name()?;
                let ty = if self.accept(Token::Colon) {
                    let t = self.ty()?;
                    self.expect(Token::Equal, "'=' after type")?;
                    Some(t)
                } else {
                    self.expect(Token::Define, "':='")?;
                    None
                };
                self.newlines();
                let init = self.expr(0)?;
                self.end_stmt()?;
                StmtKind::Decl {
                    name,
                    ty,
                    mutable,
                    init,
                }
            } else if mutable {
                return Err(Error::new(
                    "E0105",
                    span,
                    "write 'var name := value' or 'var name: i32 = value'",
                ));
            } else if matches!(self.peek(), Token::Name(_))
                && self.tokens.get(self.pos + 1).map(|t| &t.token) == Some(&Token::Equal)
            {
                let (name, _) = self.name()?;
                self.take();
                self.newlines();
                let value = self.expr(0)?;
                self.end_stmt()?;
                StmtKind::Assign(name, value)
            } else {
                let value = self.expr(0)?;
                if self.accept(Token::Equal) {
                    self.newlines();
                    let rhs = self.expr(0)?;
                    self.end_stmt()?;
                    StmtKind::AssignIndex(value, rhs)
                } else {
                    let tail = self.tail_position();
                    self.end_stmt()?;
                    if tail {
                        StmtKind::Tail(value)
                    } else {
                        StmtKind::Expr(value)
                    }
                }
            }
        };
        Ok(Stmt { kind, span })
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Error> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(Error::new(
                "E0104",
                self.span(),
                "expression nesting is too deep",
            ));
        }
        self.newlines();
        let t = self.take();
        let span = t.span;
        let mut left = match t.token {
            Token::Number(n) => Expr {
                kind: ExprKind::Number(n),
                span,
            },
            Token::String(s) => Expr {
                kind: ExprKind::String(s),
                span,
            },
            Token::Rune(r) => Expr {
                kind: ExprKind::Rune(r),
                span,
            },
            Token::Name(n) if n == "true" || n == "false" => Expr {
                kind: ExprKind::Bool(n == "true"),
                span,
            },
            Token::Name(n) => Expr {
                kind: ExprKind::Name(n),
                span,
            },
            Token::Op(op @ ("-" | "!")) => Expr {
                kind: ExprKind::Unary(op, Box::new(self.expr(7)?)),
                span,
            },
            Token::Left => {
                let saved = self.record_literals;
                self.record_literals = true;
                let e = self.expr(0)?;
                self.expect(Token::Right, "')'")?;
                self.record_literals = saved;
                e
            }
            Token::Square => {
                let kind = if self.accept(Token::EndSquare) {
                    ExprKind::Array(Vec::new())
                } else {
                    let first = self.expr(0)?;
                    if self.accept(Token::Semi) {
                        let len = self.array_len()?;
                        self.expect(Token::EndSquare, "']'")?;
                        ExprKind::Repeat(Box::new(first), len)
                    } else {
                        let mut values = vec![first];
                        while self.accept(Token::Comma) {
                            if *self.peek() == Token::EndSquare {
                                break;
                            }
                            values.push(self.expr(0)?);
                        }
                        self.expect(Token::EndSquare, "']'")?;
                        ExprKind::Array(values)
                    }
                };
                Expr { kind, span }
            }
            _ => return Err(Error::new("E0106", span, "expected an expression")),
        };
        loop {
            if self.record_literals
                && *self.peek() == Token::Open
                && matches!(left.kind, ExprKind::Name(_) | ExprKind::Member(_, _))
            {
                self.take();
                self.newlines();
                let mut fields = Vec::new();
                while *self.peek() != Token::Close {
                    let (name, span) = self.name()?;
                    self.expect(Token::Colon, "':' after field name")?;
                    let value = self.expr(0)?;
                    fields.push((name, value, span));
                    self.newlines();
                    if !self.accept(Token::Comma) {
                        break;
                    }
                    self.newlines();
                }
                let end = self.span().end;
                self.expect(Token::Close, "'}'")?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end,
                    },
                    kind: ExprKind::Record(Box::new(left), fields),
                };
                continue;
            }
            if self.accept(Token::Square) {
                let index = self.expr(0)?;
                let end = self.span().end;
                self.expect(Token::EndSquare, "']'")?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end,
                    },
                    kind: ExprKind::Index(Box::new(left), Box::new(index)),
                };
                continue;
            }
            if self.accept(Token::Dot) {
                let (member, span) = self.name()?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end: span.end,
                    },
                    kind: ExprKind::Member(Box::new(left), member),
                };
                continue;
            }
            if self.accept(Token::Left) {
                let saved = self.record_literals;
                self.record_literals = true;
                let mut args = Vec::new();
                while *self.peek() != Token::Right {
                    args.push(self.expr(0)?);
                    if !self.accept(Token::Comma) {
                        break;
                    }
                }
                let end = self.span().end;
                self.expect(Token::Right, "')'")?;
                self.record_literals = saved;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end,
                    },
                    kind: ExprKind::Call(Box::new(left), args),
                };
                continue;
            }
            if *self.peek() == Token::Range && min == 0 {
                self.take();
                let end = self.expr(1)?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end: end.span.end,
                    },
                    kind: ExprKind::Range(Box::new(left), Box::new(end)),
                };
                continue;
            }
            let Token::Op(op) = self.peek() else { break };
            let op = *op;
            let prec = match op {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" => 3,
                "<" | ">" | "<=" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if prec < min {
                break;
            }
            self.take();
            let right = self.expr(prec + 1)?;
            left = Expr {
                span: Span {
                    start: left.span.start,
                    end: right.span.end,
                },
                kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
            };
        }
        self.depth -= 1;
        Ok(left)
    }
}

pub fn parse(source: &str) -> Result<Program, Error> {
    let mut types = Types::default();
    let module = parse_module(source, &mut types)?;
    Ok(Program {
        functions: module.functions,
        imports: module.imports,
        records: module.records,
        types,
    })
}

pub fn parse_module(source: &str, types: &mut Types) -> Result<Module, Error> {
    parse_module_at(source, types, 0)
}
pub fn parse_module_at(source: &str, types: &mut Types, module: usize) -> Result<Module, Error> {
    let mut parser = Parser {
        tokens: lex(source)?,
        pos: 0,
        depth: 0,
        types,
        cases: false,
        module,
        record_literals: true,
    };
    let mut functions = Vec::new();
    let mut imports = Vec::new();
    let mut records = Vec::new();
    parser.separators();
    while *parser.peek() != Token::Eof {
        if parser.is_word("import") {
            imports.push(parser.import()?);
        } else {
            let tagged = parser.accept(Token::At);
            if tagged {
                let (tag, span) = parser.name()?;
                if tag != "test" {
                    return Err(Error::new(
                        "E0108",
                        span,
                        format!("unknown tag '@{tag}'; available tag: @test"),
                    ));
                }
                parser.newlines();
            }
            let mut cases = if tagged && *parser.peek() == Token::Open {
                parser.cases = true;
                let body = parser.block()?;
                parser.cases = false;
                parser.newlines();
                Some(body)
            } else {
                None
            };
            let public = parser.is_word("pub");
            if public {
                parser.take();
            }
            if parser.is_word("struct") {
                if tagged {
                    return Err(Error::new(
                        "E0108",
                        parser.span(),
                        "@test applies to a function",
                    ));
                }
                records.push(parser.record_decl(public)?);
                parser.separators();
                continue;
            }
            let mut function = parser.function(public)?;
            if tagged && cases.is_none() {
                function.test = Some(function.name.clone());
            }
            if let Some(body) = &mut cases {
                lower_cases(body, &function.name);
                let tests = Function {
                    public: false,
                    test: Some(function.name.clone()),
                    name: format!("$cases{}", functions.len()),
                    params: Vec::new(),
                    ret: Type::Unit,
                    body: std::mem::take(body),
                    span: function.span,
                };
                functions.push(function);
                functions.push(tests);
            } else {
                functions.push(function);
            }
        }
        parser.separators();
    }
    Ok(Module {
        functions,
        imports,
        records,
    })
}

fn implicit_return(body: &mut [Stmt]) {
    if let Some(last) = body.last_mut() {
        let kind = std::mem::replace(&mut last.kind, StmtKind::Block(Vec::new()));
        last.kind = match kind {
            StmtKind::Tail(value) => StmtKind::Return(Some(value)),
            StmtKind::If(c, mut a, mut b) => {
                implicit_return(&mut a);
                implicit_return(&mut b);
                StmtKind::If(c, a, b)
            }
            StmtKind::Block(mut b) => {
                implicit_return(&mut b);
                StmtKind::Block(b)
            }
            other => other,
        };
    }
}

fn lower_cases(body: &mut [Stmt], target: &str) {
    for stmt in body {
        match &mut stmt.kind {
            StmtKind::If(_, a, b) => {
                lower_cases(a, target);
                lower_cases(b, target);
            }
            StmtKind::While(_, b) | StmtKind::For(_, _, b) | StmtKind::Block(b) => {
                lower_cases(b, target)
            }
            StmtKind::Case(_, _) => {
                let StmtKind::Case(args, expected) =
                    std::mem::replace(&mut stmt.kind, StmtKind::Block(Vec::new()))
                else {
                    unreachable!()
                };
                let span = stmt.span;
                let actual = Expr {
                    span,
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            span,
                            kind: ExprKind::TestTarget(target.into()),
                        }),
                        args,
                    ),
                };
                let equal = Expr {
                    span,
                    kind: ExprKind::Binary("==", Box::new(actual), Box::new(expected)),
                };
                stmt.kind = StmtKind::Expr(Expr {
                    span,
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            span,
                            kind: ExprKind::Name("assert".into()),
                        }),
                        vec![equal],
                    ),
                });
            }
            _ => {}
        }
    }
}
