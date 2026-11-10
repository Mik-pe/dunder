use std::{fs, os::unix::fs::PermissionsExt, process::Command};

struct Project(std::path::PathBuf);
impl Project {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("dunder-grant-{label}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn source(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn check(&self) -> Result<dunder::modules::Checked, dunder::modules::Diagnostic> {
        dunder::modules::check_path(self.0.join("main.ddr"))
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn privileged_entry_delegates_independent_read_and_write_grants() {
    let p = Project::new("delegate");
    p.source(
        "main.ddr",
        r#"
capability filesystem
import std.fs
import worker
fn main() -> i32 {
    input := fs.grant_read("source")
    output := fs.grant_write("destination")
    worker.copy(&input, &output)
}
"#,
    );
    p.source(
        "worker.ddr",
        r#"
import std.fs
pub fn copy(src: &fs.ReadFile, dst: &fs.WriteFile) -> i32 {
    data := fs.read(&src)
    if !data.ok { return 1 }
    if !fs.write(&dst, data.value).ok { return 2 }
    0
}
"#,
    );
    let checked = p.check().unwrap_or_else(|e| panic!("{}", e.render()));
    let binary = p.0.join("copy");
    fs::write(&binary, checked.compile().unwrap()).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    p.source("source", "hämtad\n");
    assert!(Command::new(binary)
        .current_dir(&p.0)
        .status()
        .unwrap()
        .success());
    assert_eq!(
        fs::read_to_string(p.0.join("destination")).unwrap(),
        "hämtad\n"
    );
}

#[test]
fn imported_handles_are_opaque_and_factories_require_declared_authority() {
    let p = Project::new("reject");
    for body in [
        "f := fs.grant_read(\"file\")",
        "factory := fs.grant_read",
        "f := fs.WriteFile { path: \"file\" }",
        "p := src.path",
        "src.path = \"other\"",
        "change(&mut src.path)",
        "r := fs.write(&src, \"hello\")",
    ] {
        p.source("main.ddr", &format!("import std.fs\nfn change(s: &mut string) {{}}\nfn use_handle(src: &mut fs.ReadFile) {{ {body} }}\nfn main() {{}}"));
        let error = p.check().err().unwrap_or_else(|| panic!("accepted {body}"));
        assert!(
            matches!(error.error.code, "E0292" | "E0201"),
            "{}",
            error.render()
        );
    }
}
