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
    pub fn check_with_bindings(
        mut self,
        reports: &mut Vec<check::BindingInfo>,
        target: Option<crate::target::Target>,
    ) -> Result<Checked, Diagnostic> {
        if let Some(target) = target {
            self.types.set_pointer_bytes(target.pointer_bytes());
        }
        match check::check_reporting(
            self.modules,
            self.types,
            check::Mode::Program,
            Some(reports),
        ) {
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
    pub fn check(self) -> Result<Checked, Diagnostic> {
        self.check_mode(check::Mode::Program)
    }
    pub fn check_for(mut self, target: crate::target::Target) -> Result<Checked, Diagnostic> {
        self.types.set_pointer_bytes(target.pointer_bytes());
        self.check()
    }
    pub fn check_tests_for(mut self, target: crate::target::Target) -> Result<Checked, Diagnostic> {
        self.types.set_pointer_bytes(target.pointer_bytes());
        self.check_tests()
    }
    pub fn check_tests(self) -> Result<Checked, Diagnostic> {
        self.check_mode(check::Mode::Test)
    }
    fn check_mode(self, mode: check::Mode) -> Result<Checked, Diagnostic> {
        match check::check_modules(self.modules, self.types, mode) {
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
    pub fn compile_for(&self, target: crate::target::Target) -> Result<Vec<u8>, Error> {
        if self.program.types.pointer_bytes() != target.pointer_bytes() {
            return Err(Error::new("E0300", Span { start: 0, end: 0 },
                "checked pointer width differs from output target; check this module with Loaded::check_for(target)"));
        }
        crate::native::compile_for(&self.program, target)
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
    bundled: HashMap<String, usize>,
}
impl Loader {
    fn standard(&mut self, name: &str, source: &str) -> Result<usize, Error> {
        if let Some(id) = self.bundled.get(name) {
            return Ok(*id);
        }
        let id = self.sources.files.len();
        self.bundled.insert(name.to_owned(), id);
        let text = source.to_owned();
        let module =
            syntax::parse_module_at(&text, &mut self.types, id).map_err(|e| e.in_file(id))?;
        self.sources.files.push(Source {
            path: PathBuf::from(format!(
                "<stdlib/{}.ddr>",
                name.strip_prefix("std/").unwrap_or(name)
            )),
            text,
        });
        self.modules.push(check::Module {
            filesystem: module.filesystem,
            functions: module.functions,
            imports: Vec::new(),
            records: module.records,
            enums: module.enums,
            standard_sys: name == "sys",
        });
        self.imports(id, module.imports, None)?;
        Ok(id)
    }
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
        let module = syntax::parse_module_at(&self.sources.files[id].text, &mut self.types, id)
            .map_err(|e| e.in_file(id))?;
        self.modules.push(check::Module {
            filesystem: module.filesystem,
            functions: module.functions,
            imports: Vec::new(),
            records: module.records,
            enums: module.enums,
            standard_sys: false,
        });
        self.imports(id, module.imports, path.parent())?;
        Ok(id)
    }
    fn imports(
        &mut self,
        id: usize,
        imports: Vec<syntax::Import>,
        base: Option<&Path>,
    ) -> Result<(), Error> {
        for import in imports {
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
                    "omit the file suffix in imports; write import math or import utils.math",
                )
                .in_file(id));
            }
            let name = logical_name(&import.path);
            let target = if let Some(source) = standard_source(name) {
                self.standard(name, source)?
            } else {
                let base = base.ok_or_else(|| {
                    Error::new(
                        "E0401",
                        import.span,
                        "bundled libraries may only import bundled modules",
                    )
                    .in_file(id)
                })?;
                let target_path = base.join(logical).with_extension("ddr");
                self.visit(&target_path, Some((id, import.span)))?
            };
            self.modules[id].imports.push(check::Import {
                name: import.alias,
                target,
                span: import.span,
            });
        }
        Ok(())
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

fn logical_name(name: &str) -> &str {
    if name == "std/sys" {
        "sys"
    } else {
        name
    }
}
const STANDARD_MODULES: &[(&str, &str)] =
    include!(concat!(env!("OUT_DIR"), "/standard_modules.rs"));

fn standard_source(name: &str) -> Option<&'static str> {
    let name = if name == "sys" {
        name
    } else {
        name.strip_prefix("std/")?
    };
    STANDARD_MODULES
        .iter()
        .find_map(|&(module, source)| (module == name).then_some(source))
}
