use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static SERIAL: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "dunder-incremental-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, file: &str, source: &str) {
        fs::write(self.0.join(file), source).unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn verified_build(&self, target: &str) -> String {
        self.ok(&["build", "main.ddr", "--target", target, "-o", "clean"]);
        let out = self.ok(&[
            "build",
            "main.ddr",
            "--target",
            target,
            "-o",
            "warm",
            "--incremental",
            "--timings",
        ]);
        assert_eq!(
            fs::read(self.0.join("clean")).unwrap(),
            fs::read(self.0.join("warm")).unwrap()
        );
        String::from_utf8(out.stderr).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fragments_relocate_calls_branches_and_shared_literals_after_edits() {
    let p = Project::new();
    p.write(
        "main.ddr",
        "import lib\nfn main(){print(lib.first(2));print(lib.second(3))}",
    );
    for target in ["arm64-macos", "arm64-linux", "x86_64-linux"] {
        let source = "pub fn first(x:i32)->string{if x==2{return \"shared\"};return \"a\"}\npub fn second(y:i32)->string{if y==3{return \"shared\"};return \"b\"}";
        p.write("lib.ddr", source);
        assert!(p.verified_build(target).contains("functions-compiled=3"));
        assert!(p.verified_build(target).contains("functions-reused=3"));
        p.write(
            "lib.ddr",
            &source.replacen(
                "return \"shared\"",
                "if x>0{return \"new\"};return \"other\"",
                1,
            ),
        );
        assert!(p.verified_build(target).contains("functions-reused=2"));
    }
}

#[test]
fn corrupt_fragments_are_discarded_and_inline_callees_invalidate_callers() {
    let p = Project::new();
    p.write(
        "main.ddr",
        "fn add(x:i32)->i32{x+1} fn main(){print(add(41))}",
    );
    p.verified_build("arm64-macos");
    p.write(
        "main.ddr",
        "fn add(x:i32)->i32{x+2} fn main(){print(add(41))}",
    );
    assert!(p
        .verified_build("arm64-macos")
        .contains("functions-compiled=2"));
    for file in fs::read_dir(p.0.join("build/.dunder-cache/functions")).unwrap() {
        fs::write(file.unwrap().path(), b"incomplete").unwrap();
    }
    assert!(p
        .verified_build("arm64-macos")
        .contains("functions-compiled=2"));
    assert!(p
        .verified_build("arm64-macos")
        .contains("functions-reused=2"));
}

#[test]
fn affected_uses_transitive_bodies_and_retries_failures() {
    let p = Project::new();
    p.write("main.ddr","import lib\n@test fn stable(){assert(true)} @test fn dependent(){f:=lib.value;assert(f()==42)}");
    let args = ["test", "main.ddr", "--affected"];
    p.write("lib.ddr", "pub fn value()->i32{42}");
    assert!(String::from_utf8_lossy(&p.ok(&args).stdout).contains("2 passed"));
    assert!(String::from_utf8_lossy(&p.ok(&args).stdout).contains("2 unaffected"));
    p.write("lib.ddr", "pub fn value()->i32{41}");
    for _ in 0..2 {
        let out = p.run(&args);
        assert_eq!(out.status.code(), Some(1));
        let lines = String::from_utf8(out.stdout).unwrap();
        assert!(
            lines.contains("1 unaffected") && lines.contains("::dependent"),
            "{lines}"
        );
    }
}

#[test]
fn agent_mode_emits_machine_readable_spans_and_inferred_types() {
    let p = Project::new();
    let name = "odd\"name.ddr";
    p.write(name, "fn main(){title:=\"abc\"}");
    let out = String::from_utf8(p.ok(&["check", name, "--agent"]).stdout).unwrap();
    assert!(out.contains("\"name\":\"title\",\"type\":\"string\""));
    assert!(out.contains("odd\\\"name.ddr"));
    p.write(name, "import missing\nfn main(){}");
    let err = p.run(&["check", name, "--agent"]);
    assert_eq!(err.status.code(), Some(1));
    let line = String::from_utf8(err.stdout).unwrap();
    assert!(
        line.contains("\"code\":\"E0400\"")
            && line.contains("\"start\":")
            && line.lines().count() == 1
    );
    assert!(err.stderr.is_empty());
}

#[test]
fn affected_reexecutes_tests_that_observe_system_state() {
    let p = Project::new();
    p.write(
        "main.ddr",
        "import sys\n@test fn fixture(){assert(sys.read_text(\"fixture.txt\").value==\"ok\")}",
    );
    p.write("fixture.txt", "ok");
    let args = ["test", "main.ddr", "--affected"];
    assert!(String::from_utf8_lossy(&p.ok(&args).stdout).contains("main.ddr::fixture"));
    p.write("fixture.txt", "changed");
    assert_eq!(p.run(&args).status.code(), Some(1));
}
