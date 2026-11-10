use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Work(PathBuf);
impl Work {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dunder-enum-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn write(&self, n: &str, s: &str) {
        fs::write(self.0.join(n), s).unwrap();
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
    fn native(source: &str) -> std::process::Output {
        let w = Self::new();
        w.write("main.ddr", source);
        w.cli(&["run", "main.ddr"])
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(o: &std::process::Output) {
    assert!(
        o.status.success(),
        "{:?}\n{}\n{}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn enum_declarations_constructors_nominality_and_match_coverage_are_checked() {
    for (s, code) in [
        ("enum Empty{} fn main(){}", "E0270"),
        ("enum E{A,A} fn main(){}", "E0270"),
        ("enum E{A=1,B=1} fn main(){}", "E0270"),
        ("enum E{A=2147483648} fn main(){}", "E0270"),
        ("enum E{A=2147483647,B} fn main(){}", "E0270"),
        ("enum E{A(E)} fn main(){}", "E0270"),
        ("enum E{A(())} fn main(){}", "E0270"),
        ("enum E{A} fn main(){x:=E.Missing}", "E0271"),
        ("enum E{A(i32)} fn main(){x:=E.A}", "E0207"),
        ("enum E{A(i32)} fn main(){x:=E.A(true)}", "E0201"),
        ("enum E{A} fn main(){x:=.A}", "E0271"),
        ("enum E{A} enum F{A} fn f(e:E){} fn main(){f(F.A)}", "E0201"),
        (
            "enum E{A,B} fn f(e:E){match e{.A=>{}}} fn main(){}",
            "E0272",
        ),
        (
            "enum E{A,B} fn f(e:E){match e{.A=>{},.A=>{},.B=>{}}} fn main(){}",
            "E0272",
        ),
        (
            "enum E{A(i32)} fn f(e:E){match e{.A=>{}}} fn main(){}",
            "E0272",
        ),
        (
            "enum E{A(i32)} fn f(e:E){match e{.A(x)=>{x=1}}} fn main(){}",
            "E0208",
        ),
        (
            "enum E{A} fn f(e:E){match e{_=>{},.A=>{}}} fn main(){}",
            "E0272",
        ),
        ("fn main(){match 1{_=>{}}}", "E0272"),
        ("enum E{A} fn main(){x:=E{}}", "E0240"),
        ("enum E{A(i32)} fn main(){x:=E(1)}", "E0271"),
        ("enum E{A(i32)} fn main(){x:=[E.A(1)]}", "E0220"),
    ] {
        let e = dunder::check(s)
            .err()
            .unwrap_or_else(|| panic!("accepted {s}"));
        assert_eq!(e.code, code, "{s}: {}", e.message);
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
fn value_enums_keep_numeric_values_in_structs_arrays_stack_arguments_and_cases() {
    let o = Work::native(
        r#"
enum Kind{Unknown=-1,File=4,Directory,Link=9}
struct Entry{kind:Kind,size:i64}
@test{(Kind.File)->4;(.Directory)->5}
fn ordinal(k:Kind)->i32{i32(k)}
fn ninth(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,k:Kind)->Kind{k}
fn label(k:Kind)->string{match k{.Unknown=>"unknown",.File=>"file",.Directory=>"directory",.Link=>"link"}}
fn main(){
 var k:Kind=.File;k=.Directory;assert(k==Kind.Directory);assert(k!=.File)
 assert(i32(Kind(-1))==-1);assert(i64(Kind.Link)==9)
 var e:=Entry{kind:.File,size:99};e.kind=.Link;assert(e.kind==Kind.Link)
 values:[Kind;3]=[.Unknown,.File,.Directory];for value in values{print(i32(value))}
 print(label(ninth(1,2,3,4,5,6,7,8,.Link)))
}
"#,
    );
    success(&o);
    assert_eq!(o.stdout, b"-1\n4\n5\nlink\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn tagged_enums_copy_payloads_and_return_through_recursion_and_callbacks() {
    let o = Work::native(
        r#"
struct Point{x:i32,y:f64}
enum Error{Missing,Other(i32)}
enum Value{Empty,Text(string),Pair(i32,f32),Point(Point),Array([i32;3]),Error(Error)}
enum Job{Callback(fn(i32)->i32)}
fn twice(x:i32)->i32{x*2}
fn run_job(job:Job)->i32{match job{.Callback(f)=>f(21)}}
fn make(n:i32)->Value{if n==0{.Empty}else{.Text("text "+n.str())}}
fn invoke(f:fn(i32)->Value,n:i32)->Value{f(n)}
fn recurse(n:i32,v:Value)->Value{if n==0{v}else{recurse(n-1,v)}}
fn ninth(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,v:Value)->Value{v}
fn score(v:Value)->i32{
 match v{
 .Empty=>0,
 .Text(text)=>text.len,
 .Pair(a,b)=>a+i32(b),
 .Point(p)=>p.x+i32(p.y),
 .Array(a)=>a[0]+a[1]+a[2],
 .Error(error)=>{match error{.Missing=>-1,.Other(code)=>code}},
 }
}
fn main(){
 original:=invoke(make,12);var changed:=original;changed=.Pair(3,4.0)
 assert(score(original)==7);assert(score(changed)==7)
 assert(score(recurse(50,Value.Point(Point{x:3,y:4.0})))==7)
 assert(score(Value.Array([1,2,3]))==6)
 assert(score(Value.Error(Error.Other(42)))==42)
    assert(score(ninth(1,2,3,4,5,6,7,8,Value.Pair(3,4.0)))==7)
 assert(score(make(0))==0)
    assert(run_job(.Callback(twice))==42)
 var count:=0
 for i in 0..4{match make(i){.Empty=>{continue},.Text(text)=>{count=count+1},_=>{assert(false)}}}
 assert(count==3)
 match original{.Text(text)=>{changed=.Empty;assert(text=="text 12")},_=>{assert(false)}}
 print(score(original));print(score(changed))
}
"#,
    );
    success(&o);
    assert_eq!(o.stdout, b"7\n0\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn enums_preserve_identity_across_imports_and_private_variants_stay_private() {
    let w = Work::new();
    w.write("types.ddr","pub enum Reply{Text(string),Code(i32)} enum Secret{Hidden}\npub fn value()->Reply{.Text(\"hello\")} pub fn hidden()->Secret{.Hidden}");
    w.write("main.ddr","import types\nfn main(){match types.value(){types.Reply.Text(s)=>{print(s)},.Code(n)=>{print(n)}}}");
    let o = w.cli(&["run", "main.ddr"]);
    success(&o);
    assert_eq!(o.stdout, b"hello\n");
    w.write(
        "main.ddr",
        "import types\nfn main(){match types.hidden(){.Hidden=>{}}}",
    );
    let o = w.cli(&["check", "main.ddr"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("E0403"));
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn invalid_integer_to_enum_conversion_has_a_defined_status() {
    let o = Work::native("enum Kind{A=1,B=9} fn main(){x:=Kind(2)}");
    assert_eq!(o.status.code(), Some(108));
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn enum_return_values_work_with_real_filesystem_results() {
    let w = Work::new();
    w.write("input", "Hej 🌍");
    w.write(
        "main.ddr",
        r#"
import sys
enum Read{Ok(string),Err(i32)}
fn read(path:string)->Read{r:=sys.read_text(path);if r.ok{.Ok(r.value)}else{.Err(r.error)}}
fn main(){
 match read("input"){.Ok(text)=>{print(text)},.Err(error)=>{print(error);assert(false)}}
 match read("missing"){.Ok(_)=>{assert(false)},.Err(error)=>{assert(error==2)}}
}
"#,
    );
    let o = w.cli(&["run", "main.ddr"]);
    success(&o);
    assert_eq!(o.stdout, "Hej 🌍\n".as_bytes());
}
