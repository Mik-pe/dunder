use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dunder-language-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rejects(source: &str, code: &str) {
    match dunder::check(source) {
        Err(e) => assert_eq!(e.code, code, "{}", e.message),
        Ok(_) => panic!("incorrectly accepted: {source}"),
    }
}

#[test]
fn static_types_and_immutable_bindings_are_enforced() {
    for (src, code) in [
        ("fn main() { x := 1; x = 2 }", "E0208"),
        ("fn main() { x: bool = 1 }", "E0201"),
        ("fn main() { var x := 1; x = true }", "E0201"),
        ("fn f(x: i64) {} fn main() { x := 1; f(x) }", "E0201"),
        ("fn main() { x: i32 = 2147483648 }", "E0204"),
        ("fn main() { x: i64 = 9223372036854775808 }", "E0204"),
        ("fn main() { if 1 { print(1) } }", "E0201"),
        ("fn main() { if true { x := 1 }; print(x) }", "E0205"),
        (
            "fn f() -> i32 { if true { return 1 } } fn main() {}",
            "E0213",
        ),
        (
            "fn f(a: i32) -> i32 { return a } fn main() { print(f()) }",
            "E0207",
        ),
        ("fn main() { print(true + false) }", "E0206"),
        ("fn main() { break }", "E0210"),
        ("fn main() { x := 1; x := 2 }", "E0203"),
        ("fn main() { x := print(1) }", "E0202"),
        ("fn f(x: i32) { x = 1 } fn main() {}", "E0208"),
    ] {
        rejects(src, code);
    }
}

#[test]
fn source_positions_and_multiline_calls() {
    let source = "fn main() {\n    ålder := true\n    x: i32 = ålder\n}\n";
    let e = dunder::check(source).err().unwrap();
    assert!(e
        .render("bad.dn", source)
        .starts_with("bad.dn:3:14: error[E0201]"));
    dunder::check(
        "fn plus(a:i32,b:i32)->i32{return a+b}\nfn main(){x:=plus(\n 1,\n 2,\n); print(x)}",
    )
    .unwrap_or_else(|e| panic!("{}", e.message));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native(source: &str) -> std::process::Output {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::new();
    let path = dir.0.join("program");
    let image =
        dunder::compile(source).unwrap_or_else(|e| panic!("{}", e.render("test.dn", source)));
    fs::write(&path, &image).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let signed = Command::new("/usr/bin/codesign")
        .args(["--verify"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    Command::new(&path).env_clear().output().unwrap()
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn compiled_examples_and_runtime_control_flow() {
    for (source,expected,status) in [
        (include_str!("../examples/hello.dn"),"42\ntrue\n",0),
        (include_str!("../examples/fibonacci.dn"),"6765\ntrue\n",0),
        (include_str!("../examples/functions.dn"),"42\n42\n-42\n",0),
        ("fn fact(n:i32)->i32{if n<=1{return 1}else{return n*fact(n-1)}} fn main()->i32{print(fact(6));return 7}","720\n",7),
        ("fn main(){var x:=0; var total:=0; while x<10{x=x+1; if x==3{continue}; if x==8{break}; total=total+x}; print(total)}","25\n",0),
        ("fn side()->bool{print(9);return true} fn main(){print(false&&side());print(true||side());print(!true)}","false\ntrue\nfalse\n",0),
        ("fn main(){x:i64=-9223372036854775808;print(x);print(-7/3);print(-7%3);print(i64(2147483647)+1)}","-9223372036854775808\n-2\n-1\n2147483648\n",0),
        ("fn eight(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32)->i32{return a+b+c+d+e+f+g+h} fn main(){print(eight(1,2,3,4,5,6,7,8))}","36\n",0),
        ("fn main(){x:=40;{x:=2;print(x)};print(x);print(2<3);print(2!=3);print(2>=3);print(3>2)}","2\n40\ntrue\ntrue\nfalse\ntrue\n",0),
    ] {
        let result=native(source); assert_eq!(result.status.code(),Some(status),"{source}"); assert_eq!(String::from_utf8_lossy(&result.stdout),expected,"{source}");
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn integer_checks_apply_to_computed_values() {
    for (source, status) in [
        ("fn main(){x:=2147483647;print(x+1)}", 101),
        ("fn main(){x: i64=9223372036854775807;print(x+1)}", 101),
        ("fn main(){x: i64=9223372036854775807;print(x*2)}", 101),
        ("fn main(){x:=50000;print(x*x)}", 101),
        ("fn main(){x: i32=-2147483648;print(-x)}", 101),
        ("fn main(){x: i32=-2147483648;print(x/-1)}", 101),
        ("fn main(){x: i64=2147483648;print(i32(x))}", 101),
        ("fn main(){x:=0;print(1/x)}", 102),
    ] {
        let out = native(source);
        assert_eq!(out.status.code(), Some(status), "{source}");
    }
}

#[test]
fn native_output_is_reproducible() {
    let source = "fn main() { print(6 * 7) }";
    let a = dunder::compile(source).unwrap();
    let b = dunder::compile(source).unwrap();
    assert_eq!(a, b);
}

#[test]
fn cli_manifest_and_failed_build_preserve_previous_binary() {
    let dir = Dir::new();
    fs::write(
        dir.0.join("dunder.toml"),
        "name = \"demo\"\nroot = \"main.dn\"\n",
    )
    .unwrap();
    let src = dir.0.join("main.dn");
    fs::write(&src, "fn main(){print(42)}").unwrap();
    let compiler = env!("CARGO_BIN_EXE_dunder");
    let first = Command::new(compiler)
        .args(["build", "--timings"])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let output = dir.0.join("build/arm64-macos/demo");
    let before = fs::read(&output).unwrap();
    fs::write(&src, "fn main(){x:bool=42}").unwrap();
    let bad = Command::new(compiler)
        .arg("build")
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert_eq!(fs::read(&output).unwrap(), before);
    assert!(String::from_utf8_lossy(&bad.stderr).contains("error[E0201]"));
    let valid = "fn main(){print(42)}";
    fs::write(&src, valid).unwrap();
    let overwrite = Command::new(compiler)
        .args(["build", "main.dn", "-o", "main.dn"])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert!(!overwrite.status.success());
    assert_eq!(fs::read_to_string(&src).unwrap(), valid);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_integer_and_comparison_results_match_independent_reference() {
    let mut source = String::new();
    for ty in ["i32", "i64"] {
        source.push_str(&format!("fn probe_{ty}(a:{ty},b:{ty}) {{ print(a+b); print(a-b); print(a*b); print(a/b); print(a%b); print(a<b); print(a<=b); print(a>b); print(a>=b); print(a==b); print(a!=b) }}\n"));
    }
    source.push_str("fn main(){\n");
    let mut expected = String::new();
    let mut seed = 0x8d3b47u64;
    for ty in ["i32", "i64"] {
        let bound = if ty == "i32" { 10000i64 } else { 1000000000 };
        for _ in 0..32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let a = (seed as i64) % (bound + 1);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let b = (seed as i64) % bound;
            let b = if b == 0 { 1 } else { b };
            source.push_str(&format!("probe_{ty}({a},{b})\n"));
            for n in [a + b, a - b, a * b, a / b, a % b] {
                expected.push_str(&format!("{n}\n"));
            }
            for n in [a < b, a <= b, a > b, a >= b, a == b, a != b] {
                expected.push_str(&format!("{n}\n"));
            }
        }
    }
    source.push('}');
    let out = native(&source);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn function_signatures_returns_and_main_are_checked() {
    for (source, code) in [
        ("fn main()->bool{return true}", "E0212"),
        ("fn main(arg:i32){}", "E0212"),
        ("fn helper(){}", "E0212"),
        ("fn main(){} fn main(){}", "E0203"),
        ("fn f()->i32{return} fn main(){}", "E0201"),
        ("fn f(){return 1} fn main(){}", "E0201"),
        ("fn f()->bool{return 1} fn main(){}", "E0201"),
        ("fn f(n:i32)->i32{return n} fn main(){f(true)}", "E0201"),
        (
            "fn f(n:i32)->i32{return n} fn main(){g:fn(i32)->i64=f}",
            "E0201",
        ),
        (
            "fn f(n:i32)->i32{return n} fn choose()->fn(i64)->i32{return f} fn main(){}",
            "E0201",
        ),
        (
            "fn f(n:i32)->i32{return n} fn use(g:fn(i64)->i32){} fn main(){use(f)}",
            "E0201",
        ),
        ("fn main(){g:=1;print(g(2))}", "E0206"),
        ("fn f(){} fn main(){g:=f;g=f}", "E0208"),
        ("fn f(){} fn main(){print(f)}", "E0206"),
        ("fn f(){} fn main(){print(f==f)}", "E0206"),
    ] {
        rejects(source, code);
    }
    let source = "fn f(n:i32)->i32{return n} fn main(){g:fn(i32)->i64=f}";
    let e = dunder::check(source).err().unwrap();
    assert_eq!(e.message, "expected fn(i32) -> i64, found fn(i32) -> i32");
    dunder::check("fn main()->(){return}").unwrap_or_else(|e| panic!("{}", e.message));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn callbacks_returned_functions_and_nested_calls_execute() {
    for (source,expected) in [
        ("fn add(a:i32,b:i32)->i32{return a+b} fn mark(n:i32)->i32{print(n);return n} fn factory()->fn(i32,i32)->i32{print(0);return add} fn main(){print(factory()(mark(1),mark(2)))}","0\n1\n2\n3\n"),
        ("fn plus(n:i32)->i32{return n+1} fn minus(n:i32)->i32{return n-1} fn main(){var step:=plus;print(step(2));step=minus;print(step(2));print((plus)(41))}","3\n1\n42\n"),
        ("fn show(n:i32){print(n);return} fn main(){consumer:fn(i32)=show;consumer(42)}","42\n"),
        ("fn even(n:i32)->bool{if n==0{return true};return odd(n-1)} fn odd(n:i32)->bool{if n==0{return false};return even(n-1)} fn main(){predicate:fn(i32)->bool=even;print(predicate(100));print(predicate(101))}","true\nfalse\n"),
        ("fn double(n:i32)->i32{return n*2} fn get()->fn(i32)->i32{return double} fn main(){f:=get();var i:=0;var sum:=0;while i<10000{sum=sum+f(i);i=i+1};print(sum)}","99990000\n"),
    ] {
        let out=native(source);assert!(out.status.success(),"{source}");assert_eq!(String::from_utf8_lossy(&out.stdout),expected,"{source}");
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn stack_arguments_preserve_types_alignment_and_recursion() {
    let source = r#"
fn double(n:i32)->i32{return n*2}
fn mixed(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,
         flag:bool,x:i32,wide:i64,other:bool,y:i32,transform:fn(i32)->i32)->i64 {
    if flag && !other { return wide+i64(transform(x+y))+i64(a+b+c+d+e+f+g+h) }
    return 0
}
fn walk(n:i32,a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,last:i64)->i64 {
    if n==0 { return last }
    return walk(n-1,a,b,c,d,e,f,g,last)+1
}
fn main(){
    callback:=mixed
    print(callback(1,2,3,4,5,6,7,8,true,-5,10000000000,false,7,double))
    print(walk(100,1,2,3,4,5,6,7,9000000000))
}
"#;
    let out = native(source);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "10000000040\n9000000100\n"
    );
    // Exercise a much larger argument list, including negative signed stack values.
    let params = (0..40)
        .map(|i| format!("a{i}:i64"))
        .collect::<Vec<_>>()
        .join(",");
    let args = (0..40)
        .map(|i| (i as i64 - 20).to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sum = (0..40)
        .map(|i| format!("a{i}"))
        .collect::<Vec<_>>()
        .join("+");
    let source = format!("fn sum({params})->i64{{return {sum}}} fn main(){{print(sum({args}))}}");
    let out = native(&source);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "-20\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn run_uses_main_and_propagates_its_exit_status() {
    let dir = Dir::new();
    let source = dir.0.join("program.dn");
    fs::write(&source, "fn main()->i32{print(42);return 7}").unwrap();
    let compiler = env!("CARGO_BIN_EXE_dunder");
    let output = Command::new(compiler)
        .args(["run", "program.dn", "-o", "program"])
        .current_dir(&dir.0)
        .env("PATH", "/no-external-tools")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
    let before = fs::read(dir.0.join("program")).unwrap();
    fs::write(&source, "fn main()->bool{return true}").unwrap();
    let bad = Command::new(compiler)
        .args(["run", "program.dn", "-o", "program"])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(bad.stdout.is_empty());
    assert_eq!(fs::read(dir.0.join("program")).unwrap(), before);
    fs::write(&source, "fn main(){print(true);return}").unwrap();
    let unit = Command::new(compiler)
        .args(["run", "program.dn"])
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert!(unit.status.success());
    assert_eq!(String::from_utf8_lossy(&unit.stdout), "true\n");
}
