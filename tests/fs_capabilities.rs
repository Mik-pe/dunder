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

#[test]
#[cfg(all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos")))]
fn selfbuilt_compiler_checks_grants_in_the_originating_module() {
    let work = Project::new("stage2");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiler_source = root.join(if cfg!(target_os = "macos") {
        "selfhost/main.ddr"
    } else {
        "selfhost/linux.ddr"
    });
    let build = |compiler: &std::path::Path, source: &std::path::Path, name: &str| {
        Command::new(compiler)
            .current_dir(&work.0)
            .arg("build")
            .arg(source)
            .arg("-o")
            .arg(work.0.join(name))
            .output()
            .unwrap()
    };
    for (compiler, name) in [
        (
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_dunder")),
            "stage1",
        ),
        (work.0.join("stage1"), "stage2"),
    ] {
        let result = build(&compiler, &compiler_source, name);
        assert!(result.status.success(), "{result:?}");
    }

    work.source("main.ddr", "capability filesystem\nimport std.fs\nimport worker\nfn main(){src:=fs.grant_read(\"source\");dst:=fs.grant_write(\"dest\"); worker.copy(&src,&dst)}");
    let header = "import std.fs\npub fn copy(src:&fs.ReadFile,dst:&fs.WriteFile)";
    let compiler = work.0.join("stage2");
    let source = work.0.join("main.ddr");
    work.source(
        "worker.ddr",
        &format!("{header} {{r:=fs.read(&src);assert(r.ok);assert(fs.write(&dst,r.value).ok)}}"),
    );
    let ok = build(&compiler, &source, "copy");
    assert!(ok.status.success(), "{ok:?}");
    work.source("source", "delegated");
    assert!(Command::new(work.0.join("copy"))
        .current_dir(&work.0)
        .status()
        .unwrap()
        .success());
    assert_eq!(
        fs::read_to_string(work.0.join("dest")).unwrap(),
        "delegated"
    );
    for (tail, message) in [
        ("{ x := src.path }", "opaque struct"),
        ("{ src.path = \"other\" }", "opaque struct"),
        (
            "{ x := fs.WriteFile { path: \"forged\" } }",
            "opaque struct",
        ),
        ("{ x := fs.grant_read(\"other\") }", "capability filesystem"),
        (
            "{}\nfn dead() { factory := fs.grant_write }",
            "capability filesystem",
        ),
        (
            "{}\nfn dead() { unsafe { p:=ptr(0) } }",
            "capability unsafe",
        ),
    ] {
        work.source("worker.ddr", &format!("{header} {tail}"));
        let result = build(&compiler, &source, "rejected");
        assert!(
            !result.status.success() && String::from_utf8_lossy(&result.stderr).contains(message),
            "{tail} {result:?}"
        );
    }
}
