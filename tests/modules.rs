use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-modules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn check(&self) -> dunder::modules::Checked {
        dunder::modules::check_path(self.0.join("main.ddr"))
            .unwrap_or_else(|e| panic!("{}", e.render()))
    }
    fn error(&self, code: &str) -> dunder::modules::Diagnostic {
        let error = match dunder::modules::check_path(self.0.join("main.ddr")) {
            Err(e) => e,
            Ok(_) => panic!("expected {code}"),
        };
        assert_eq!(error.error.code, code, "{}", error.render());
        error
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap()
    }
    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    fn execute(&self, image: &[u8]) -> String {
        use std::os::unix::fs::PermissionsExt;
        let output = self.0.join("program");
        // A new inode avoids reusing macOS's signature cache across compilations.
        let tmp = self.0.join("program.new");
        fs::write(&tmp, image).unwrap();
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).unwrap();
        fs::rename(&tmp, &output).unwrap();
        #[cfg(target_os = "macos")]
        {
            let verify = Command::new("/usr/bin/codesign")
                .arg("--verify")
                .arg(&output)
                .output()
                .unwrap();
            assert!(
                verify.status.success(),
                "{}",
                String::from_utf8_lossy(&verify.stderr)
            );
        }
        let run = Command::new(output).env_clear().output().unwrap();
        assert!(run.status.success());
        String::from_utf8(run.stdout).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compile(project: &dunder::modules::Checked) -> Vec<u8> {
    project
        .compile()
        .unwrap_or_else(|e| panic!("{}", project.render(&e)))
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn actual_module_example_builds_without_external_tools() {
    let project = Project::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/modules/main.ddr");
    let checked = dunder::modules::check_path(&root).unwrap_or_else(|e| panic!("{}", e.render()));
    assert_eq!(checked.sources.files.len(), 3);
    assert_eq!(project.execute(&compile(&checked)), "42\ntrue\n42\n");
    let result = project.cli(&["run", root.to_str().unwrap(), "--timings"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"42\ntrue\n42\n");
    assert!(String::from_utf8_lossy(&result.stderr).contains("modules=3"));
}

#[test]
fn privacy_and_explicit_namespaces_are_enforced() {
    let project = Project::new();
    project.write(
        "math.ddr",
        "fn hidden()->i32{return 7}\npub fn value()->i32{return hidden()}\n",
    );
    for (source, code, message) in [
        (
            "import math\nfn main(){print(math.hidden())}",
            "E0403",
            "math.hidden",
        ),
        (
            "import math\nfn main(){print(math.absent())}",
            "E0404",
            "absent",
        ),
        ("import math\nfn main(){print(value())}", "E0205", "value"),
        (
            "import math\nfn main(){math:=1;print(math.value())}",
            "E0405",
            "local value",
        ),
        ("import math\nfn main(){x:=math}", "E0205", "math"),
        (
            "import math\nfn main(){print(other.value())}",
            "E0404",
            "other",
        ),
    ] {
        project.write("main.ddr", source);
        let error = project.error(code);
        let rendered = error.render();
        assert!(rendered.contains(message), "{rendered}");
        assert!(rendered.contains("main.ddr:2:"), "{rendered}");
    }
}

#[test]
fn import_names_paths_and_diagnostics_are_checked() {
    let project = Project::new();
    project.write("math.ddr", "pub fn value()->i32{return 42}");
    project.write("foo-bar.ddr", "pub fn value()->i32{return 42}");
    for (source, code) in [
        ("import math\nimport math\nfn main(){}", "E0402"),
        ("import math\nfn math(){}\nfn main(){}", "E0402"),
        ("import math as fn\nfn main(){}", "E0401"),
        ("import \"foo-bar\"\nfn main(){}", "E0401"),
        ("import \"math.ddr\"\nfn main(){}", "E0401"),
        ("import \"/absolute/math\"\nfn main(){}", "E0401"),
        ("import \"\"\nfn main(){}", "E0401"),
        ("import \"math\nfn main(){}", "E0004"),
    ] {
        project.write("main.ddr", source);
        project.error(code);
    }
    project.write("main.ddr", "\nimport absent\nfn main(){}");
    let missing = project.error("E0400").render();
    assert!(missing.contains("main.ddr:2:1:"), "{missing}");
    assert!(missing.contains("absent.ddr"));
    project.write("main.ddr", "import math\nfn main(){}");
    project.write("math.ddr", "pub fn broken()->bool{\n    return 42\n}\n");
    let error = project.error("E0201").render();
    assert!(error.contains("math.ddr:2:12:"), "{error}");
    project.write("math.ddr", "pub fn broken(\n");
    let error = project.error("E0101").render();
    assert!(error.contains("math.ddr:"), "{error}");
    assert!(dunder::check("import math\nfn main(){}")
        .err()
        .unwrap()
        .message
        .contains("source path"));
}

#[test]
fn bare_import_paths_preserve_spelling_aliases_and_statement_boundaries() {
    let source = "import std/map\r\nimport ../shared/value as common; import ./sys as local\nimport å-dir/value-name as value // trailing comment\nimport /* leading comment */ util/math /* trailing comment */\nimport 'legacy/path' as legacy\nimport std/text";
    let parsed = dunder::syntax::parse(source).unwrap();
    let paths: Vec<_> = parsed
        .imports
        .iter()
        .map(|import| (import.path.as_str(), import.alias.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            ("std/map", "map"),
            ("../shared/value", "common"),
            ("./sys", "local"),
            ("å-dir/value-name", "value"),
            ("util/math", "math"),
            ("legacy/path", "legacy"),
            ("std/text", "text"),
        ]
    );
    for import in &parsed.imports {
        assert_eq!(&source[import.span.start..import.span.end], "import");
    }
    // Path lexing is contextual; '/' and '-' retain their expression meaning.
    dunder::check("fn main(){print(12 / 3 - 1);print(12/3-1)}").unwrap();
}

#[test]
fn malformed_bare_import_paths_fail_before_loading_a_different_module() {
    let project = Project::new();
    project.write("math.ddr", "pub fn value()->i32{42}");
    project.write("foo-bar.ddr", "pub fn value()->i32{42}");
    for (path, code) in [
        ("math/", "E0401"),
        ("math//hidden", "E0401"),
        ("math/*hidden*/", "E0401"),
        ("math\\hidden", "E0401"),
        ("/absolute/math", "E0401"),
        (".", "E0401"),
        ("..", "E0401"),
        ("math/..", "E0401"),
        ("math.ddr", "E0401"),
        ("foo-bar", "E0401"),
        ("math / value", "E0107"),
        ("math value", "E0107"),
        ("", "E0107"),
    ] {
        project.write("main.ddr", &format!("\nimport {path}\nfn main(){{}}"));
        let error = project.error(code).render();
        assert!(error.contains("main.ddr:2:"), "{path}: {error}");
    }
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn bare_imports_load_relative_paths_bundled_modules_and_aliases_natively() {
    let project = Project::new();
    project.write(
        "main.ddr",
        r#"import std/text
import std/map as counts
import sys
import ./sys as local
import util/predicates
import å-dir/value-name as value
fn main(){
    var m := counts.new()
    counts.put(&mut m, "answer", i64(predicates.answer()))
    print(counts.get_or(&m, "answer", 0))
    print(text.trim(" å 🌍 "))
    print(local.value() + value.answer())
    print(sys.pid() > 0)
}
"#,
    );
    project.write("sys.ddr", "pub fn value()->i32{20}");
    project.write(
        "util/predicates.ddr",
        "import ../shared/value as common\npub fn answer()->i32{common.answer()}",
    );
    project.write("shared/value.ddr", "pub fn answer()->i32{42}");
    project.write("å-dir/value-name.ddr", "pub fn answer()->i32{22}");
    let checked = project.check();
    assert_eq!(project.execute(&compile(&checked)), "42\nå 🌍\n42\ntrue\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn aliases_private_helpers_and_same_named_functions_stay_separate() {
    let project = Project::new();
    project.write("main.ddr","import a\nimport b\nimport \"foo-bar\" as more\nfn main(){print(a.value());print(b.value());print(more.value())}");
    project.write(
        "a.ddr",
        "fn helper()->i32{return 1} pub fn value()->i32{return helper()}",
    );
    project.write(
        "b.ddr",
        "fn helper()->i32{return 2} pub fn value()->i32{return helper()}",
    );
    project.write("foo-bar.ddr", "pub fn value()->i32{return 3}");
    assert_eq!(project.execute(&compile(&project.check())), "1\n2\n3\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn diamonds_relative_paths_and_symlinks_load_each_file_once() {
    use std::os::unix::fs::symlink;
    let project = Project::new();
    project.write("main.ddr","import \"parts/left\"\nimport \"parts/right\"\nfn main(){print(left.value()+right.value())}");
    project.write(
        "parts/left.ddr",
        "import \"../shared/value\" as common\npub fn value()->i32{return common.answer()}",
    );
    project.write(
        "parts/right.ddr",
        "import \"../shared/link\" as common\npub fn value()->i32{return common.answer()}",
    );
    project.write("shared/value.ddr", "pub fn answer()->i32{return 42}");
    symlink("value.ddr", project.0.join("shared/link.ddr")).unwrap();
    let checked = project.check();
    assert_eq!(checked.sources.files.len(), 4);
    assert_eq!(checked.program.functions.len(), 4);
    assert_eq!(project.execute(&compile(&checked)), "84\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn import_cycles_support_cross_module_mutual_recursion() {
    let project = Project::new();
    project.write(
        "main.ddr",
        "import even\nfn main(){print(even.test(100));print(even.test(101))}",
    );
    project.write(
        "even.ddr",
        "import odd\npub fn test(n:i32)->bool{if n==0{return true};return odd.test(n-1)}",
    );
    project.write(
        "odd.ddr",
        "import even\npub fn test(n:i32)->bool{if n==0{return false};return even.test(n-1)}",
    );
    let checked = project.check();
    assert_eq!(checked.sources.files.len(), 3);
    assert_eq!(project.execute(&compile(&checked)), "true\nfalse\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn function_types_are_shared_across_module_boundaries() {
    let project = Project::new();
    project.write(
        "wide.ddr",
        "fn double(x:i64)->i64{return x*2} pub fn factory()->fn(i64)->i64{return double}",
    );
    project.write(
        "apply.ddr",
        "pub fn run(f:fn(i64)->i64,value:i64)->i64{return f(value)}",
    );
    project.write("main.ddr","import wide\nimport apply\nfn main(){f:fn(i64)->i64=wide.factory();print(apply.run(f,5000000000))}");
    assert_eq!(project.execute(&compile(&project.check())), "10000000000\n");
    project.write(
        "main.ddr",
        "import wide\nfn main(){f:fn(i32)->i32=wide.factory()}",
    );
    let error = project.error("E0201");
    assert_eq!(
        error.error.message,
        "expected fn(i32) -> i32, found fn(i64) -> i64"
    );
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn builds_are_reproducible_relocatable_and_see_dependency_edits() {
    let a = Project::new();
    let b = Project::new();
    for project in [&a, &b] {
        project.write(
            "main.ddr",
            "import math\nimport math as other\nfn main(){print(math.value()+other.value())}",
        );
        project.write("math.ddr", "pub fn value()->i32{return 21}");
        project.write("unused.ddr", "this file must never be parsed");
    }
    let first = a.check();
    assert_eq!(first.sources.files.len(), 2);
    let image = compile(&first);
    assert_eq!(image, compile(&b.check()));
    assert_eq!(a.execute(&image), "42\n");
    a.write("math.ddr", "pub fn value()->i32{return 20}");
    let changed = compile(&a.check());
    assert_ne!(image, changed);
    assert_eq!(a.execute(&changed), "40\n");
}

#[test]
fn only_root_main_is_an_entrypoint_and_failed_builds_preserve_all_sources() {
    let project = Project::new();
    project.write("main.ddr", "import math\nfn main(){print(math.value())}");
    let dependency = "fn main()->i32{return 7} pub fn value()->i32{return 42}";
    project.write("math.ddr", dependency);
    project.write("dunder.toml", "name = \"demo\"\nroot = \"main.ddr\"\n");
    let build = project.cli(&["build"]);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let before = fs::read(project.0.join(format!(
        "build/{}/demo",
        dunder::target::Target::host().unwrap().name()
    )))
    .unwrap();
    let overwrite = project.cli(&["build", "-o", "math.ddr"]);
    assert!(!overwrite.status.success());
    assert_eq!(
        fs::read_to_string(project.0.join("math.ddr")).unwrap(),
        dependency
    );
    project.write("math.ddr", "pub fn value()->i32{return true}");
    let bad = project.cli(&["run"]);
    assert!(!bad.status.success());
    assert!(bad.stdout.is_empty());
    assert_eq!(
        fs::read(project.0.join(format!(
            "build/{}/demo",
            dunder::target::Target::host().unwrap().name()
        )))
        .unwrap(),
        before
    );
    project.write("math.ddr", dependency);
    project.write("main.ddr", "import math\nfn helper(){}");
    project.error("E0212");
}
