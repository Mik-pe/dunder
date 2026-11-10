pub mod check;
pub mod modules;
pub mod native;
pub mod syntax;

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct Error {
    pub file: usize,
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

impl Error {
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            file: 0,
            code,
            span,
            message: message.into(),
        }
    }
    pub fn in_file(mut self, file: usize) -> Self {
        self.file = file;
        self
    }
    pub fn render(&self, path: &str, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let line = source[..start].bytes().filter(|b| *b == b'\n').count() + 1;
        let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
        let col = source[line_start..start].chars().count() + 1;
        let text = source[line_start..].lines().next().unwrap_or("");
        format!(
            "{path}:{line}:{col}: error[{}]: {}\n  {text}\n  {}^",
            self.code,
            self.message,
            " ".repeat(col - 1)
        )
    }
}

pub fn check(source: &str) -> Result<check::Program, Error> {
    check::check(syntax::parse(source)?)
}

pub fn compile(source: &str) -> Result<Vec<u8>, Error> {
    let program = check(source)?;
    native::compile(&program).map_err(|e| Error::new("E0300", Span { start: 0, end: 0 }, e))
}
