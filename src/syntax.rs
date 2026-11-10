use crate::{Error, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I32,
    I64,
    Bool,
    Unit,
    Function(usize),
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
            Self::Unit => "()",
            Self::Function(_) => "fn",
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
}
impl Types {
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
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Name(String),
    Number(i128),
    Left,
    Right,
    Open,
    Close,
    Colon,
    Define,
    Equal,
    Comma,
    Semi,
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
                    '{' => Token::Open,
                    '}' => Token::Close,
                    ':' => Token::Colon,
                    '=' => Token::Equal,
                    ',' => Token::Comma,
                    ';' => Token::Semi,
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
    Return(Option<Expr>),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
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
    Name(String),
    Call(Box<Expr>, Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
}

struct Parser {
    tokens: Vec<Lexeme>,
    pos: usize,
    depth: usize,
    types: Types,
}
impl Parser {
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
        let (n, span) = self.name()?;
        match n.as_str() {
            "i32" => Ok(Type::I32),
            "i64" => Ok(Type::I64),
            "bool" => Ok(Type::Bool),
            _ => Err(Error::new(
                "E0103",
                span,
                format!("unknown type '{n}'; available types: i32, i64, bool"),
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
    fn function(&mut self) -> Result<Function, Error> {
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
        Ok(Function {
            name,
            params,
            ret,
            body: self.block()?,
            span,
        })
    }
    fn stmt(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        let kind = if self.is_word("if") {
            self.take();
            let condition = self.expr(0)?;
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
        } else if self.is_word("while") {
            self.take();
            let condition = self.expr(0)?;
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
                self.end_stmt()?;
                StmtKind::Expr(value)
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
                let e = self.expr(0)?;
                self.expect(Token::Right, "')'")?;
                e
            }
            _ => return Err(Error::new("E0106", span, "expected an expression")),
        };
        loop {
            if self.accept(Token::Left) {
                let mut args = Vec::new();
                while *self.peek() != Token::Right {
                    args.push(self.expr(0)?);
                    if !self.accept(Token::Comma) {
                        break;
                    }
                }
                let end = self.span().end;
                self.expect(Token::Right, "')'")?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end,
                    },
                    kind: ExprKind::Call(Box::new(left), args),
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
    let mut parser = Parser {
        tokens: lex(source)?,
        pos: 0,
        depth: 0,
        types: Types::default(),
    };
    let mut functions = Vec::new();
    parser.separators();
    while *parser.peek() != Token::Eof {
        functions.push(parser.function()?);
        parser.separators();
    }
    Ok(Program {
        functions,
        types: parser.types,
    })
}
