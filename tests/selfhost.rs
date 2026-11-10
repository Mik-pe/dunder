#![cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Work(PathBuf);
impl Work {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dunder-selfhost-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn bootstrap(&self) -> PathBuf {
        let output = self.0.join("dunder-ddr");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("selfhost/main.ddr");
        let result = Command::new(env!("CARGO_BIN_EXE_dunder"))
            .arg("build")
            .arg(source)
            .arg("-o")
            .arg(&output)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap();
        success(&result);
        output
    }
    fn build(&self, compiler: &Path, source: &str) -> Output {
        fs::write(self.0.join("input.ddr"), source).unwrap();
        Command::new(compiler)
            .args(["build", "input.ddr", "-o", "program"])
            .current_dir(&self.0)
            .env_clear()
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap()
    }
    fn run(&self) -> Output {
        Command::new(self.0.join("program"))
            .env_clear()
            .output()
            .unwrap()
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(o: &Output) {
    assert!(
        o.status.success(),
        "{:?}\n{}\n{}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
#[test]
fn dunder_written_pipeline_matches_bootstrap_for_checked_scalar_programs() {
    let w = Work::new();
    let compiler = w.bootstrap();
    let mut source = String::from(
        r#"
fn fib(n:i32)->i64 {if n<2 {return i64(n)}; fib(n-1)+fib(n-2)}
fn classify(x:i32)->i32 {if x<0 {-1} else if x==0 {0} else {1}}
fn greet(x:string)->string{x}
fn main(){
 print(fib(20));print(classify(-2));print(classify(0));print(classify(3))
 print(greet("å 🌍\0tail"));print("")
 var sum:=0
 for i in -3..10{if i==2{continue};if i==8{break};sum=sum+i}
 print(sum)
 var i:=0;while i<4{i=i+1};print(i)
 print(false && (1/0==0));print(true || (1/0==0));print(!false)
 minimum:i64=-9223372036854775808;print(minimum)
 maximum:i64=9223372036854775807;print(maximum)
 a:i64=1234567890123;b:i64=-9876543;print(a*b)
 print(-17/5);print(-17%5)
 assert(true)
"#,
    );
    let mut seed = 71u32;
    for _ in 0..160 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = (seed % 20001) as i32 - 10000;
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = (seed % 199) as i32 + 1;
        source+=&format!("print(({a})+({b}));print(({a})*({b}));print(({a})/({b}));print(({a})%({b}));print(({a})<({b}));\n");
    }
    source += "}\n";
    // The i64 multiplication above is deliberately in range for both compilers.
    source = source.replace("1234567890123", "123456789");
    success(&w.build(&compiler, &source));
    let actual = w.run();
    success(&actual);
    fs::write(w.0.join("reference"), dunder::compile(&source).unwrap()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(w.0.join("reference"), fs::Permissions::from_mode(0o755)).unwrap();
    let expected = Command::new(w.0.join("reference"))
        .env_clear()
        .output()
        .unwrap();
    success(&expected);
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.status.code(), expected.status.code());
    success(
        &Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict"])
            .arg(w.0.join("program"))
            .output()
            .unwrap(),
    );
    let first = fs::read(w.0.join("program")).unwrap();
    success(&w.build(&compiler, &source));
    assert_eq!(first, fs::read(w.0.join("program")).unwrap());
}
#[test]
fn dunder_written_pipeline_rejects_errors_and_preserves_existing_output_and_sources() {
    let w = Work::new();
    let compiler = w.bootstrap();
    success(&w.build(&compiler, "fn main(){print(42)}"));
    let previous = fs::read(w.0.join("program")).unwrap();
    for source in [
        "fn main(){x:=1;x=2}",
        "fn main(){var x:=1;x=true}",
        "fn main(){if 1{}}",
        "fn main(){break}",
        "fn main(){unknown()}",
        "fn main(){print(1) print(2)}",
        "fn f(x:i32){} fn main(){f()}",
        "fn f(x:i32){} fn main(){f(true)}",
        "fn main(){x:=1;x:=2}",
        "fn f(x:i32){x:=1} fn main(){}",
        "fn main(){i32:=1}",
        "fn f()->i32{42} fn main(){f:=1;print(f())}",
        "fn f()->i32{if true{1}else{2};return 3} fn main(){}",
        "fn f()->i32{} fn main(){}",
        "fn main(){",
        "fn main(){print(\"unterminated)}",
        "fn main(x:i32){}",
        "fn main()->i64{0}",
        "import sys\nfn main(){}",
        "struct S{x:i32} fn main(){}",
        "fn main(){var a:[]i32=[]}",
    ] {
        let result = w.build(&compiler, source);
        assert_eq!(
            result.status.code(),
            Some(1),
            "{source}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("E1000"));
        assert_eq!(previous, fs::read(w.0.join("program")).unwrap());
    }
    let original = b"fn main(){print(1)}";
    fs::write(w.0.join("input.ddr"), original).unwrap();
    fs::hard_link(w.0.join("input.ddr"), w.0.join("hard.ddr")).unwrap();
    std::os::unix::fs::symlink("input.ddr", w.0.join("link.ddr")).unwrap();
    for target in ["./input.ddr", "hard.ddr", "link.ddr"] {
        let o = Command::new(&compiler)
            .args(["build", "input.ddr", "-o", target])
            .current_dir(&w.0)
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1));
        assert_eq!(
            original,
            fs::read(w.0.join("input.ddr")).unwrap().as_slice()
        );
    }
    // Explicit evidence that this is a bootstrap stage, not a self-hosting claim.
    let own = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("selfhost/main.ddr"))
        .unwrap();
    let result = w.build(&compiler, &own);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stdout).contains("imports"));
}
#[test]
fn dunder_written_runtime_traps_agree_with_bootstrap() {
    let w = Work::new();
    let compiler = w.bootstrap();
    for (source, status) in [
        ("fn main(){var x:=2147483647;x=x+1}", 101),
        ("fn main(){var x:i64=9223372036854775807;x=x+1}", 101),
        ("fn main(){var x:i64=3037000500;x=x*x}", 101),
        ("fn main(){var x:i64=-9223372036854775808;x=-x}", 101),
        ("fn main(){var x:=0;print(1/x)}", 102),
        (
            "fn main(){var x:i64=-9223372036854775808;print(x / -1)}",
            101,
        ),
        ("fn main(){assert(false)}", 104),
    ] {
        success(&w.build(&compiler, source));
        let result = w.run();
        assert_eq!(
            result.status.code(),
            Some(status),
            "{source}\n{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
}
