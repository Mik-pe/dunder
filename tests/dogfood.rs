use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-dogfood-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) {
        fs::write(self.0.join(name), source).unwrap();
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap()
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn native(source: &str) -> std::process::Output {
        use std::os::unix::fs::PermissionsExt;
        let work = Self::new();
        let image =
            dunder::compile(source).unwrap_or_else(|e| panic!("{}", e.render("test.ddr", source)));
        let path = work.0.join("program");
        fs::write(&path, image).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let signed = Command::new("/usr/bin/codesign")
            .arg("--verify")
            .arg(&path)
            .output()
            .unwrap();
        assert!(signed.status.success());
        Command::new(path).env_clear().output().unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn collection_types_and_immutability_are_checked() {
    for (source, code) in [
        ("fn main(){a:=[1,2];a[0]=3}", "E0208"),
        ("fn f(a:[i32;2]){a[0]=3} fn main(){}", "E0208"),
        ("fn main(){a:=[1,true]}", "E0201"),
        ("fn main(){a:[i32;3]=[1,2]}", "E0201"),
        ("fn main(){a:=[1,2];print(a[true])}", "E0201"),
        ("fn main(){x:=1;print(x[0])}", "E0221"),
        ("fn main(){for i in 5{print(i)}}", "E0222"),
        ("fn main(){for i in 0..5{i=1}}", "E0208"),
        ("fn main(){for i in 0..5{};print(i)}", "E0205"),
        ("fn main(){a:=[0;65537]}", "E0220"),
        ("fn main(){a:=[]}", "E0220"),
        ("fn main(){a:=[1,2];it:=a.iter();print(it[0])}", "E0221"),
        ("fn main(){a:=[[1,2];2]}", "E0220"),
        ("fn main(){n:=3;a:=[0;n]}", "E0220"),
        ("fn f(a:[i32;3]){} fn main(){f([1,2])}", "E0201"),
        ("fn main(){for i in 0..true{}}", "E0201"),
    ] {
        let error = dunder::check(source)
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.code, code, "{}", error.message);
    }
}

#[test]
fn tagged_test_signatures_and_assertions_are_checked() {
    for (source, code) in [
        ("@test fn wrong(x:i32){}", "E0230"),
        ("@test fn wrong()->i32{return 0}", "E0230"),
        ("@test fn wrong(){assert(1)}", "E0201"),
        ("@test fn wrong(){assert()}", "E0207"),
        ("@test fn same(){} @test fn same(){}", "E0203"),
        ("@bench fn unsupported(){}", "E0108"),
    ] {
        let error = dunder::check_tests(source)
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.code, code, "{}", error.message);
    }
    let source = "fn main(){} @test fn only_in_test_builds(){missing_name()}";
    assert!(dunder::check(source).is_ok());
    assert_eq!(dunder::check_tests(source).err().unwrap().code, "E0205");
}

#[test]
fn attached_cases_and_struct_inputs_are_statically_checked() {
    for (source, code) in [
        ("@test{(20)->true} fn f(x:i32)->i32{x/5}", "E0201"),
        ("@test{(20,5)->4} fn f(x:i32)->i32{x/5}", "E0207"),
        ("@test{(false)->4} fn f(x:i32)->i32{x/5}", "E0201"),
        (
            "struct Point{x:i32,y:i32} @test{(Point{x:3})->9} fn f(p:Point)->i32{p.x}",
            "E0241",
        ),
        (
            "struct Point{x:i32} @test{(Point{x:3,x:4})->9} fn f(p:Point)->i32{p.x}",
            "E0241",
        ),
        (
            "struct Point{x:i32} @test{(Point{y:3})->9} fn f(p:Point)->i32{p.x}",
            "E0241",
        ),
        (
            "struct Point{x:i32} @test{(Point{x:true})->9} fn f(p:Point)->i32{p.x}",
            "E0201",
        ),
        (
            "struct Point{x:i32} fn f(p:Point){p.x=1} @test fn ok(){}",
            "E0208",
        ),
        (
            "struct Point{x:i32} @test fn bad(){p:=Point{x:3};p.x=4}",
            "E0208",
        ),
        (
            "struct Point{x:i32} @test fn bad(){p:=Point{x:3};print(p.missing)}",
            "E0241",
        ),
        ("struct Point{x:i32,x:i64} @test fn ok(){}", "E0240"),
        ("struct A{x:i32} struct B{a:A} @test fn ok(){}", "E0240"),
        ("fn f(p:Unknown){} @test fn ok(){}", "E0103"),
    ] {
        let error = dunder::check_tests(source)
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.code, code, "{}", error.message);
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn tail_expressions_return_values_without_returning_from_ordinary_blocks() {
    let source = r#"
fn maximum(a:i32,b:i32)->i32{if a>b{a}else{b}}
fn identity(n:i64)->i64{n}
fn main(){
    if true{print(1)}
    print(maximum(4,9))
    {print(2)}
    print(identity(5000000000))
    print(3)
}"#;
    let out = Workspace::native(source);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"1\n9\n2\n5000000000\n3\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn struct_layout_copying_and_field_expression_order_are_native() {
    let source = r#"
struct Mixed{enabled:bool,n:i64,small:i32,ready:bool}
struct Empty{}
struct Point{x:i32,y:i32}
fn compute(p:Mixed,a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,last:Mixed)->i64{
    var copy:=p
    copy.n=copy.n+1
    if copy.enabled&&last.ready{copy.n+last.n+i64(copy.small+last.small+a+b+c+d+e+f+g)}else{0}
}
fn mark(x:i32)->i32{print(x);x}
fn consume(empty:Empty)->i32{42}
fn main(){
    original:=Mixed{enabled:true,n:10000000000,small:-3,ready:false}
    other:=Mixed{enabled:false,n:5000000000,small:7,ready:true}
    print(compute(original,1,2,3,4,5,6,7,other));print(original.n)
    point:=Point{y:mark(4),x:mark(3)}
    print(point.x*point.x+point.y*point.y)
    print(consume(Empty{}))
}"#;
    let out = Workspace::native(source);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"15000000033\n10000000000\n4\n3\n25\n42\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn tagged_case_failures_test_the_actual_function_and_boolean_expressions() {
    let work = Workspace::new();
    work.write(
        "suite.ddr",
        r#"
@test{(20)->5}
fn div_by_five(x:i32)->i32{x/5}
@test fn a_boolean_expression_can_fail(){1+1==3}
@test{()->40+2}
fn answer()->i32{42}
fn main(){print(div_by_five(20))}
"#,
    );
    let out = work.cli(&["test", "suite.ddr"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("FAIL suite.ddr::div_by_five (assertion failed)"));
    assert!(text.contains("FAIL suite.ddr::a_boolean_expression_can_fail"));
    assert!(text.contains("PASS suite.ddr::answer"));
    assert!(text.contains("1 passed, 2 failed"));
    let run = work.cli(&["run", "suite.ddr"]);
    assert!(run.status.success());
    assert_eq!(run.stdout, b"4\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn shorthand_cases_cannot_be_redirected_by_a_local_name() {
    let work = Workspace::new();
    work.write(
        "suite.ddr",
        r#"
fn other(x:i32)->i32{999}
@test{
    div_by_five:=other
    (20)->4
    div_by_five(20)==999
}
fn div_by_five(x:i32)->i32{x/5}
"#,
    );
    let out = work.cli(&["test", "suite.ddr"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("1 passed, 0 failed"));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn exported_structs_have_nominal_types_across_modules_and_callbacks() {
    let work = Workspace::new();
    work.write("geometry.ddr","pub struct Point{x:i32,y:i32}\nstruct Secret{x:i32}\npub fn squared(p:Point)->i32{p.x*p.x+p.y*p.y}");
    work.write(
        "suite.ddr",
        r#"
import geometry
fn apply(f:fn(geometry.Point)->i32,p:geometry.Point)->i32{f(p)}
@test{
    (geometry.Point{x:3,y:4})->25
    point:=geometry.Point{x:3,y:4}
    apply(geometry.squared,point)==25
}
fn squared(p:geometry.Point)->i32{geometry.squared(p)}
"#,
    );
    let out = work.cli(&["test", "suite.ddr"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("1 passed, 0 failed"));
    work.write(
        "suite.ddr",
        "import geometry\n@test fn denied(){p:=geometry.Secret{x:1}}",
    );
    let denied = work.cli(&["test", "suite.ddr"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("error[E0403]"));
    work.write("suite.ddr","import geometry\nstruct Other{x:i32,y:i32}\n@test fn wrong(){geometry.squared(Other{x:3,y:4})==25}");
    let nominal = work.cli(&["test", "suite.ddr"]);
    assert!(!nominal.status.success());
    assert!(String::from_utf8_lossy(&nominal.stderr).contains("error[E0201]"));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn collection_values_copy_and_iterator_snapshots_are_independent() {
    let source = r#"fn main(){
        original:=[1,2,3]
        var copy:=original
        copy[0]=99
        print(original[0]);print(copy[0])
        iterator:=copy.iter()
        copy[0]=7
        var sum:=0
        for n in iterator {sum=sum+n}
        print(sum)
        for n in copy {copy[1]=0;print(n)}
    }"#;
    let out = Workspace::native(source);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"1\n99\n104\n7\n2\n3\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn range_loops_cover_boundaries_continue_break_and_evaluate_bounds_once() {
    let source = r#"
fn lower()->i32{print(90);return -2}
fn upper()->i32{print(91);return 3}
fn main(){
    var sum:=0
    for i in lower()..upper(){if i==0{continue};if i==2{break};sum=sum+i}
    print(sum)
    for i in 3..0{print(999)}
    for i in -2147483648..-2147483647{print(i)}
    for i in 2147483646..2147483647{print(i)}
    var nested:=0
    for a in 0..3{for b in 0..3{if b==1{continue};nested=nested+1}}
    print(nested)
}"#;
    let out = Workspace::native(source);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"90\n91\n-2\n-2147483648\n2147483646\n6\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn arrays_preserve_layout_types_and_parameters_beyond_eight_arguments() {
    let source = r#"
fn inspect(a:[i32;3],n:i64,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,last:[i64;2])->i64{
    var copy:=a
    copy[0]=99
    return i64(copy[0])+n+i64(b+c+d+e+f+g)+last[1]
}
fn seed()->i32{print(9);return 7}
fn main(){
    a:=[1,2,3]
    b:[i64;2]=[-9223372036854775808,4294967296]
    print(inspect(a,10,1,2,3,4,5,6,b));print(a[0]);print(1+b[1])
    for n in b{print(n)}
    var flags:=[true,false,true]
    flags[1]=true
    for flag in flags{print(flag)}
    repeated:=[seed();3]
    for n in repeated{print(n)}
    empty:[i32;0]=[]
    for n in empty{print(999)}
    print(len(empty));print([1,2].len())
    for n in [3,4].iter().iter(){print(n)}
}"#;
    let out = Workspace::native(source);
    assert!(out.status.success());
    assert_eq!(out.stdout,b"4294967426\n1\n4294967297\n-9223372036854775808\n4294967296\ntrue\ntrue\ntrue\n9\n7\n7\n7\n0\n2\n3\n4\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn array_bounds_and_assertions_fail_with_defined_statuses() {
    for (source, status) in [
        ("fn main(){a:=[1,2];print(a[-1])}", 103),
        ("fn main(){a:=[1,2];print(a[2])}", 103),
        ("fn main(){var a:=[1,2];a[2]=9}", 103),
        ("fn main(){a:[i32;0]=[];print(a[0])}", 103),
        ("fn main(){assert(false)}", 104),
    ] {
        let out = Workspace::native(source);
        assert_eq!(out.status.code(), Some(status), "{source}");
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn real_dogfood_programs_compile_run_and_self_check() {
    let work = Workspace::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("dogfood");
    for (file, expected) in [
        ("recursion.ddr", "6765\n21\n"),
        ("sorting.ddr", "45\n9\n"),
        ("iterators.ddr", "36\n20\n0\n"),
        ("arena/main.ddr", "28\n"),
        ("structs.ddr", "25\n32\n3\n"),
        ("numeric/main.ddr", "5.0\n2.0\ntrue\n"),
        ("filesystem.ddr", "18\nfilesystem ok\n"),
        ("collections.ddr", "-2\n-1\n0\n1\n13\n3\n"),
        ("utf8.ddr", "bytes=25\nrunes=20\nwords=5\n🌍 Hej världen\n"),
        (
            "tokenizer.ddr",
            "fn|hälsa|(|namn|)|{|print|(|\"Hej 🌍\"|)|}\n",
        ),
    ] {
        let path = root.join(file);
        let out = work.cli(&["run", path.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{file}");
    }
    let path = root.join("tests.ddr");
    let out = work.cli(&["test", path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("28 passed, 0 failed"));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test_command_isolates_failures_skips_main_and_reports_captured_output() {
    let work = Workspace::new();
    work.write(
        "suite.ddr",
        r#"
fn main(){assert(false)}
@test fn first(){print(999);assert(true)}
@test fn failing(){print(123);assert(false)}
@test fn after_failure(){assert(true)}
"#,
    );
    let out = work.cli(&["test", "suite.ddr"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("PASS suite.ddr::first"));
    assert!(text.contains("FAIL suite.ddr::failing (assertion failed)"));
    assert!(text.contains("stdout: 123"));
    assert!(text.contains("PASS suite.ddr::after_failure"));
    assert!(text.contains("2 passed, 1 failed"));
    assert!(!text.contains("999"));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test_command_times_out_and_continues_to_the_next_test() {
    let work = Workspace::new();
    work.write(
        "suite.ddr",
        "@test fn hangs(){while true{}}\n@test fn after_timeout(){assert(true)}",
    );
    let out = work.cli(&["test", "suite.ddr", "--timeout-ms", "250"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("hangs (timeout)"));
    assert!(text.contains("PASS suite.ddr::after_timeout"));
    assert!(text.contains("1 passed, 1 failed"));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn test_mode_needs_no_main_deduplicates_imports_and_honors_the_manifest() {
    let work = Workspace::new();
    work.write("dunder.toml", "name = \"suite\"\nroot = \"suite.ddr\"\n");
    work.write("suite.ddr","import helper\nimport helper as other\n@test fn root(){assert(helper.answer()==other.answer())}");
    work.write(
        "helper.ddr",
        "pub fn answer()->i32{return 42}\n@test fn dependency(){assert(answer()==42)}",
    );
    let out = work.cli(&["test"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("2 passed, 0 failed"));
    work.write("suite.ddr", "fn main(){assert(true)}");
    let empty = work.cli(&["test"]);
    assert!(empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stdout).contains("0 passed, 0 failed"));
    work.write(
        "dunder.toml",
        "name = \"suite\"\nroot = \"main.ddr\"\ntest_root = \"suite.ddr\"\n",
    );
    work.write("main.ddr", "fn main(){print(42)}");
    work.write("suite.ddr", "@test fn selected_by_manifest(){true}");
    let selected = work.cli(&["test"]);
    assert!(selected.status.success());
    assert!(String::from_utf8_lossy(&selected.stdout).contains("1 passed, 0 failed"));
    let main = work.cli(&["run"]);
    assert!(main.status.success());
    assert_eq!(main.stdout, b"42\n");
}
