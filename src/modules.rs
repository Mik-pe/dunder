//! File loading is separate from parsing/type checking. Only explicit imports
//! are read; canonical paths deduplicate diamonds, aliases and import cycles.
use crate::{check, syntax, Error, Span};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub struct Source {
    pub path: PathBuf,
    pub text: String,
}
#[derive(Default)]
pub struct Sources {
    pub files: Vec<Source>,
}
pub struct Diagnostic {
    pub error: Error,
    pub sources: Sources,
}
impl Diagnostic {
    pub fn render(&self) -> String {
        if let Some(source) = self.sources.files.get(self.error.file) {
            self.error
                .render(&source.path.display().to_string(), &source.text)
        } else {
            format!("error[{}]: {}", self.error.code, self.error.message)
        }
    }
}

pub struct Loaded {
    pub sources: Sources,
    modules: Vec<check::Module>,
    types: syntax::Types,
}
pub struct Checked {
    pub sources: Sources,
    pub program: check::Program,
}
impl Loaded {
    pub fn check(self) -> Result<Checked, Diagnostic> {
        match check::check_modules(self.modules, self.types) {
            Ok(program) => Ok(Checked {
                sources: self.sources,
                program,
            }),
            Err(error) => Err(Diagnostic {
                error,
                sources: self.sources,
            }),
        }
    }
}
impl Checked {
    pub fn compile(&self) -> Result<Vec<u8>, Error> {
        crate::native::compile(&self.program)
            .map_err(|e| Error::new("E0300", Span { start: 0, end: 0 }, e))
    }
    pub fn render(&self, error: &Error) -> String {
        let source = &self.sources.files[error.file];
        error.render(&source.path.display().to_string(), &source.text)
    }
}

#[derive(Default)]
struct Loader {
    sources: Sources,
    modules: Vec<check::Module>,
    types: syntax::Types,
    paths: HashMap<PathBuf, usize>,
}
impl Loader {
    fn visit(&mut self, path: &Path, origin: Option<(usize, Span)>) -> Result<usize, Error> {
        let io_error = |e: std::io::Error| {
            let (file, span) = origin.unwrap_or((0, Span { start: 0, end: 0 }));
            Error::new(
                "E0400",
                span,
                format!("cannot load module {}: {e}", path.display()),
            )
            .in_file(file)
        };
        let path = fs::canonicalize(path).map_err(io_error)?;
        if let Some(id) = self.paths.get(&path) {
            return Ok(*id);
        }
        let text = fs::read_to_string(&path).map_err(io_error)?;
        let id = self.sources.files.len();
        self.paths.insert(path.clone(), id);
        self.sources.files.push(Source {
            path: path.clone(),
            text,
        });
        let module = syntax::parse_module(&self.sources.files[id].text, &mut self.types)
            .map_err(|e| e.in_file(id))?;
        self.modules.push(check::Module {
            functions: module.functions,
            imports: Vec::new(),
        });
        for import in module.imports {
            let logical = Path::new(&import.path);
            if import.path.is_empty() || logical.is_absolute() || logical.file_name().is_none() {
                return Err(Error::new(
                    "E0401",
                    import.span,
                    "import a relative module name or path",
                )
                .in_file(id));
            }
            if logical.extension().is_some() {
                return Err(Error::new(
                    "E0401",
                    import.span,
                    "omit the file suffix in imports; write import math or import \"utils/math\"",
                )
                .in_file(id));
            }
            let target_path = path.parent().unwrap().join(logical).with_extension("dn");
            let target = self.visit(&target_path, Some((id, import.span)))?;
            self.modules[id].imports.push(check::Import {
                name: import.alias,
                target,
                span: import.span,
            });
        }
        Ok(id)
    }
}

pub fn load(root: impl AsRef<Path>) -> Result<Loaded, Diagnostic> {
    let mut loader = Loader::default();
    match loader.visit(root.as_ref(), None) {
        Ok(_) => Ok(Loaded {
            sources: loader.sources,
            modules: loader.modules,
            types: loader.types,
        }),
        Err(error) => Err(Diagnostic {
            error,
            sources: loader.sources,
        }),
    }
}

pub fn check_path(root: impl AsRef<Path>) -> Result<Checked, Diagnostic> {
    load(root)?.check()
}
