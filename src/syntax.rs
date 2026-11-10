use crate::{Error, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
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
    Enum(usize),
    Dynamic(usize),
    Reference(usize),
}
impl Type {
    pub fn integer(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }
    pub fn float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
    pub fn numeric(self) -> bool {
        self.integer() || self.float()
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
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
            Self::Enum(_) => "enum",
            Self::Dynamic(_) => "dynamic array",
            Self::Reference(_) => "reference",
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
    enums: Vec<EnumType>,
    dynamics: Vec<Type>,
    references: Vec<(Type, bool)>,
}
#[derive(Clone, Debug)]
pub struct EnumType {
    pub name: String,
    pub module: usize,
    pub data: bool,
    pub variants: Vec<Variant>,
    pub bytes: usize,
}
#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub tag: i32,
    pub fields: Vec<RecordField>,
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
            Type::I32 | Type::Rune | Type::F32 | Type::Enum(_) => 4,
            _ => 8,
        }
    }
    pub fn bytes(self) -> usize {
        self.elem_size() * self.len
    }
}
impl Types {
    pub fn dynamic(&mut self, elem: Type, span: Span) -> Result<Type, Error> {
        if !matches!(
            elem,
            Type::I32
                | Type::I64
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::String
                | Type::Rune
                | Type::Named(_)
        ) && !self.scalar_enum(elem)
        {
            return Err(Error::new(
                "E0280",
                span,
                "dynamic array elements must be scalar values",
            ));
        }
        let index = self
            .dynamics
            .iter()
            .position(|t| *t == elem)
            .unwrap_or_else(|| {
                self.dynamics.push(elem);
                self.dynamics.len() - 1
            });
        Ok(Type::Dynamic(index))
    }
    pub fn dynamic_elem(&self, ty: Type) -> Option<Type> {
        if let Type::Dynamic(i) = ty {
            self.dynamics.get(i).copied()
        } else {
            None
        }
    }
    pub fn reference(&mut self, ty: Type, mutable: bool) -> Type {
        let index = self
            .references
            .iter()
            .position(|r| *r == (ty, mutable))
            .unwrap_or_else(|| {
                self.references.push((ty, mutable));
                self.references.len() - 1
            });
        Type::Reference(index)
    }
    pub fn referenced(&self, ty: Type) -> Option<(Type, bool)> {
        if let Type::Reference(i) = ty {
            self.references.get(i).copied()
        } else {
            None
        }
    }
    pub fn value_type(&self, ty: Type) -> Type {
        self.referenced(ty).map_or(ty, |(t, _)| t)
    }
    pub fn scalar_size(&self, ty: Type) -> usize {
        match ty {
            Type::Bool => 1,
            Type::I32 | Type::Rune | Type::F32 => 4,
            _ if self.scalar_enum(ty) => 4,
            _ => 8,
        }
    }
    pub fn declare_enum(&mut self, module: usize, name: String, data: bool) -> Type {
        let index = self.enums.len();
        self.enums.push(EnumType {
            name,
            module,
            data,
            variants: Vec::new(),
            bytes: 4,
        });
        Type::Enum(index)
    }
    pub fn enumeration(&self, ty: Type) -> Option<&EnumType> {
        if let Type::Enum(i) = ty {
            self.enums.get(i)
        } else {
            None
        }
    }
    pub fn define_enum(&mut self, ty: Type, variants: Vec<Variant>, bytes: usize) {
        if let Type::Enum(i) = ty {
            self.enums[i].variants = variants;
            self.enums[i].bytes = bytes;
        }
    }
    pub fn scalar_enum(&self, ty: Type) -> bool {
        self.enumeration(ty).is_some_and(|e| !e.data)
    }
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
        if matches!(ty, Type::Dynamic(_)) {
            return Some(24);
        }
        self.array(ty)
            .map(|a| a.bytes())
            .or_else(|| self.record(ty).map(|r| r.bytes))
            .or_else(|| self.enumeration(ty).filter(|e| e.data).map(|e| e.bytes))
    }
    pub fn array_type(&mut self, elem: Type, len: usize, span: Span) -> Result<Type, Error> {
        if !matches!(
            elem,
            Type::I32 | Type::I64 | Type::Bool | Type::String | Type::Rune | Type::F32 | Type::F64
        ) && !matches!(elem, Type::Named(_))
            && !self.scalar_enum(elem)
        {
            return Err(Error::new(
                "E0220",
                span,
                "array elements must be scalar values in this version",
            ));
        }
        let array = ArrayType { elem, len };
        if len > 65536 || (!matches!(elem, Type::Named(_)) && array.bytes() > 65536) {
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
        if let Some(elem) = self.dynamic_elem(ty) {
            return format!("[]{}", self.name(elem));
        }
        if let Some((inner, mutable)) = self.referenced(ty) {
            return format!("&{}{}", if mutable { "mut " } else { "" }, self.name(inner));
        }
        if let Some(n) = self.named_ref(ty) {
            return n.name.clone();
        }
        if let Some(r) = self.record(ty) {
            return r.name.clone();
        }
        if let Some(e) = self.enumeration(ty) {
            return e.name.clone();
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
    pub enums: Vec<Enumeration>,
}

pub struct Module {
    pub functions: Vec<Function>,
    pub imports: Vec<Import>,
    pub records: Vec<Record>,
    pub enums: Vec<Enumeration>,
}
#[derive(Debug)]
pub struct Enumeration {
    pub name: String,
    pub public: bool,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}
#[derive(Debug)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Type>,
    pub tag: Option<i128>,
    pub span: Span,
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
    ImportPath(String),
    Number(i128),
    Float(String),
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
    FatArrow,
    Amp,
    Op(&'static str),
    Eof,
}
#[derive(Clone, Debug)]
struct Lexeme {
    token: Token,
    span: Span,
}

// ASCII source needs no per-character index allocation. Unicode files retain
// byte-accurate offsets and the exact same lexer rules.
struct Codepoints<'a> {
    ascii: Option<&'a [u8]>,
    unicode: Vec<(usize, char)>,
}
impl<'a> Codepoints<'a> {
    fn new(source: &'a str) -> Self {
        if source.is_ascii() {
            Self {
                ascii: Some(source.as_bytes()),
                unicode: Vec::new(),
            }
        } else {
            Self {
                ascii: None,
                unicode: source.char_indices().collect(),
            }
        }
    }
    fn len(&self) -> usize {
        self.ascii.map_or(self.unicode.len(), |s| s.len())
    }
    fn at(&self, i: usize) -> (usize, char) {
        if let Some(s) = self.ascii {
            (i, char::from(s[i]))
        } else {
            self.unicode[i]
        }
    }
    fn get(&self, i: usize) -> Option<(usize, char)> {
        (i < self.len()).then(|| self.at(i))
    }
}

fn lex(source: &str) -> Result<Vec<Lexeme>, Error> {
    let mut out = Vec::new();
    let chars = Codepoints::new(source);
    let mut i = 0;
    let mut parens = 0usize;
    while i < chars.len() {
        let (start, c) = chars.at(i);
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
                let ch = chars.at(i).1;
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
            while i < chars.len() && chars.at(i).1 != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                let pair = (chars.at(i).1, chars.get(i + 1).map(|x| x.1));
                if pair == ('/', Some('*')) {
                    depth += 1;
                    i += 2;
                } else if pair == ('*', Some('/')) {
                    depth -= 1;
                    i += 2;
                } else {
                    if chars.at(i).1 == '\n' && parens == 0 {
                        out.push(Lexeme {
                            token: Token::Newline,
                            span: Span {
                                start: chars.at(i).0,
                                end: chars.at(i).0 + 1,
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
        // Import paths are lexical names, not arithmetic expressions. Keep the
        // full spelling (including malformed repeated/trailing slashes) so a
        // typo cannot silently turn into a different module plus a comment.
        // Whitespace separates an unquoted path from `as` and comments.
        if matches!(out.last().map(|t: &Lexeme| &t.token), Some(Token::Name(n)) if n == "import")
            && (c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
        {
            i += 1;
            while i < chars.len() && !chars.at(i).1.is_whitespace() && chars.at(i).1 != ';' {
                i += 1;
            }
            let end = chars.get(i).map_or(source.len(), |x| x.0);
            out.push(Lexeme {
                token: Token::ImportPath(source[start..end].into()),
                span: Span { start, end },
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            i += 1;
            while i < chars.len() && (chars.at(i).1.is_alphanumeric() || chars.at(i).1 == '_') {
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
            while i < chars.len() && (chars.at(i).1.is_ascii_digit() || chars.at(i).1 == '_') {
                i += 1;
            }
            let mut floating = false;
            if chars.get(i).is_some_and(|x| x.1 == '.')
                && chars.get(i + 1).is_some_and(|x| x.1.is_ascii_digit())
            {
                floating = true;
                i += 1;
                while i < chars.len() && (chars.at(i).1.is_ascii_digit() || chars.at(i).1 == '_') {
                    i += 1;
                }
            }
            if chars.get(i).is_some_and(|x| matches!(x.1, 'e' | 'E')) {
                floating = true;
                i += 1;
                if chars.get(i).is_some_and(|x| matches!(x.1, '+' | '-')) {
                    i += 1;
                }
                let first = i;
                while i < chars.len() && (chars.at(i).1.is_ascii_digit() || chars.at(i).1 == '_') {
                    i += 1;
                }
                if i == first {
                    return Err(Error::new(
                        "E0006",
                        Span {
                            start,
                            end: chars.get(i).map_or(source.len(), |x| x.0),
                        },
                        "an exponent needs decimal digits",
                    ));
                }
            }
            let end = chars.get(i).map_or(source.len(), |x| x.0);
            if floating {
                let spelling = &source[start..end];
                let parts: Vec<char> = spelling.chars().collect();
                if parts.iter().enumerate().any(|(j, c)| {
                    *c == '_'
                        && (j == 0
                            || !parts[j - 1].is_ascii_digit()
                            || !parts.get(j + 1).is_some_and(char::is_ascii_digit))
                }) {
                    return Err(Error::new(
                        "E0006",
                        Span { start, end },
                        "numeric separators must be between digits",
                    ));
                }
                out.push(Lexeme {
                    token: Token::Float(spelling.replace('_', "")),
                    span: Span { start, end },
                });
                continue;
            }
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
            ('=', Some('>')) => Some(Token::FatArrow),
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
                    '&' => Token::Amp,
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
    Match(Expr, Vec<MatchArm>),
}
#[derive(Debug)]
pub struct MatchArm {
    pub pattern: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}
#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Debug)]
pub enum ExprKind {
    Borrow(Box<Expr>, bool),
    Number(i128),
    Float(String),
    Bool(bool),
    String(String),
    Rune(char),
    Name(String),
    Variant(String),
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
        if self.accept(Token::Amp) {
            let mutable = if self.is_word("mut") {
                self.take();
                true
            } else {
                false
            };
            let inner = self.ty()?;
            if self.types.referenced(inner).is_some() || inner == Type::Unit {
                return Err(Error::new("E0280", self.span(), "reference a value type"));
            }
            return Ok(self.types.reference(inner, mutable));
        }
        if self.accept(Token::Square) {
            let span = self.span();
            if self.accept(Token::EndSquare) {
                let elem = self.ty()?;
                return self.types.dynamic(elem, span);
            }
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
            "f32" => Ok(Type::F32),
            "f64" => Ok(Type::F64),
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
    fn enum_decl(&mut self, public: bool) -> Result<Enumeration, Error> {
        let span = self.take().span;
        let (name, _) = self.name()?;
        self.newlines();
        self.expect(Token::Open, "'{'")?;
        self.separators();
        let mut variants = Vec::new();
        while *self.peek() != Token::Close {
            let (name, span) = self.name()?;
            let mut fields = Vec::new();
            if self.accept(Token::Left) {
                while *self.peek() != Token::Right {
                    fields.push(self.ty()?);
                    if !self.accept(Token::Comma) {
                        break;
                    }
                }
                self.expect(Token::Right, "')'")?;
            }
            let tag = if self.accept(Token::Equal) {
                let negative = self.accept(Token::Op("-"));
                let token = self.take();
                let Token::Number(value) = token.token else {
                    return Err(Error::new(
                        "E0270",
                        token.span,
                        "enum tags must be integer literals",
                    ));
                };
                Some(if negative { -value } else { value })
            } else {
                None
            };
            variants.push(EnumVariant {
                name,
                fields,
                tag,
                span,
            });
            if self.accept(Token::Comma) || self.accept(Token::Newline) {
                self.separators();
            } else {
                break;
            }
        }
        self.expect(Token::Close, "'}'")?;
        Ok(Enumeration {
            name,
            public,
            variants,
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
            Token::ImportPath(path) => {
                if path.split('/').any(|part| {
                    part.is_empty()
                        || !part
                            .chars()
                            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
                }) || matches!(path.rsplit('/').next(), Some("." | ".."))
                {
                    return Err(Error::new(
                        "E0401",
                        token.span,
                        "expected a relative module path without empty components; write import std/map or import ../shared/value",
                    ));
                }
                path
            }
            Token::String(path) => path,
            _ => {
                return Err(Error::new(
                    "E0107",
                    token.span,
                    "expected a module name or relative path, for example import math or import std/map",
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
        if !matches!(self.peek(), Token::Eof | Token::Newline | Token::Semi) {
            return Err(Error::new(
                "E0107",
                self.span(),
                "expected 'as name', a newline or ';' after the import path; path components must be adjacent",
            ));
        }
        self.separators();
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
        } else if self.is_word("match") {
            self.take();
            let source = self.header_expr()?;
            self.expect(Token::Open, "'{'")?;
            self.separators();
            let mut arms = Vec::new();
            while *self.peek() != Token::Close {
                let span = self.span();
                let pattern = self.header_expr()?;
                self.expect(Token::FatArrow, "'=>'")?;
                self.newlines();
                let body = if *self.peek() == Token::Open {
                    self.block()?
                } else {
                    let expr = self.expr(0)?;
                    vec![Stmt {
                        kind: StmtKind::Tail(expr),
                        span,
                    }]
                };
                arms.push(MatchArm {
                    pattern,
                    body,
                    span,
                });
                if *self.peek() == Token::Close {
                    break;
                }
                if !self.accept(Token::Comma) && !self.accept(Token::Newline) {
                    return Err(Error::new(
                        "E0100",
                        self.span(),
                        "expected ',' or newline after match arm",
                    ));
                }
                self.separators();
            }
            self.expect(Token::Close, "'}'")?;
            StmtKind::Match(source, arms)
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
            Token::Amp => {
                let mutable = if self.is_word("mut") {
                    self.take();
                    true
                } else {
                    false
                };
                Expr {
                    kind: ExprKind::Borrow(Box::new(self.expr(7)?), mutable),
                    span,
                }
            }
            Token::Dot => {
                let (name, end) = self.name()?;
                Expr {
                    kind: ExprKind::Variant(name),
                    span: Span {
                        start: span.start,
                        end: end.end,
                    },
                }
            }
            Token::Number(n) => Expr {
                kind: ExprKind::Number(n),
                span,
            },
            Token::Float(n) => Expr {
                kind: ExprKind::Float(n),
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
        enums: module.enums,
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
    let mut enums = Vec::new();
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
            if parser.is_word("enum") {
                if tagged {
                    return Err(Error::new(
                        "E0108",
                        parser.span(),
                        "@test applies to a function",
                    ));
                }
                enums.push(parser.enum_decl(public)?);
                parser.separators();
                continue;
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
        enums,
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
            StmtKind::Match(source, mut arms) => {
                for arm in &mut arms {
                    implicit_return(&mut arm.body);
                }
                StmtKind::Match(source, arms)
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
            StmtKind::Match(_, arms) => {
                for arm in arms {
                    lower_cases(&mut arm.body, target);
                }
            }
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
