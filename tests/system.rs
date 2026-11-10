use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-system-{}-{}",
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
    fn checked(&self) -> dunder::modules::Checked {
        dunder::modules::check_path(self.0.join("main.ddr"))
            .unwrap_or_else(|e| panic!("{}", e.render()))
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn run(&self, source: &str) -> std::process::Output {
        self.write("main.ddr", source);
        self.cli(&["run", "main.ddr"])
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(out: &std::process::Output) {
    assert!(
        out.status.success(),
        "status {:?}\nstdout: {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn bundled_sys_is_typed_deduplicated_and_distinct_from_explicit_local_modules() {
    let work = Workspace::new();
    work.write("sys.ddr", "pub fn answer()->i32{42}");
    work.write("main.ddr","import sys\nimport sys as other\nimport \"./sys\" as local\nfn main(){x:=sys.metadata(\".\");print(x.ok);print(other.pid());print(local.answer())}");
    assert_eq!(work.checked().sources.files.len(), 3);
    for (source, code) in [
        ("import sys\nfn main(){x:=sys.read_text(42)}", "E0201"),
        ("import sys\nfn main(){x:=sys.write_text(\"a\")}", "E0207"),
        ("import sys\nfn main(){x:i32=sys.metadata(\".\")}", "E0201"),
        ("import sys\nfn main(){__sys_read_text(\"a\")}", "E0205"),
        ("fn main(){__sys_pid()}", "E0205"),
        ("import \"sys.ddr\"\nfn main(){}", "E0401"),
    ] {
        work.write("main.ddr", source);
        let error = dunder::modules::check_path(work.0.join("main.ddr"))
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.error.code, code, "{}", error.render());
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn aggregate_returns_copy_into_caller_storage_across_recursion_callbacks_and_mixed_args() {
    let work = Workspace::new();
    let out = work.run(
        r#"
struct Point{x:i32,y:f64,label:string}
fn make(x:i32,y:f64)->Point{Point{x:x,y:y,label:"point"}}
fn apply(f:fn(i32,f64)->Point)->Point{f(3,4.0)}
fn selected()->fn(i32,f64)->Point{make}
fn recurse(n:i32,p:Point)->Point{if n==0{p}else{var copy:=p;copy.x=copy.x+1;recurse(n-1,copy)}}
fn values()->[i32;4]{[1,2,3,4]}
fn empty()->[i32;0]{[]}
fn many(a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,i:i32,p:Point)->Point{
    var copy:=p;copy.x=copy.x+a+b+c+d+e+f+g+h+i;copy
}
fn main(){
    original:=apply(selected());var changed:=recurse(20,original);changed.y=9.0
    assert(original.x==3&&original.y==4.0);assert(changed.x==23&&changed.y==9.0)
    assert(many(1,2,3,4,5,6,7,8,9,original).x==48)
    assert(make(5,6.0).label=="point")
    var array:=values();copy:=array;array[0]=99;assert(copy[0]==1);assert(empty().len==0)
    for n in 0..100{p:=make(n,f64(n));assert(p.x==n&&p.y==f64(n))}
    print(original.label);print(original.y)
}
"#,
    );
    success(&out);
    assert_eq!(out.stdout, b"point\n4.0\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_filesystem_round_trip_metadata_append_rename_and_cleanup() {
    let work = Workspace::new();
    let out = work.run(
        r#"
import sys
fn main(){
    assert(sys.pid()>0)
    assert(sys.mkdir("data").ok)
    assert(sys.metadata("data").is_dir)
    assert(sys.write_text("data/å 🌍.txt","Hej åäö 🌍\n").ok)
    assert(sys.append_text("data/å 🌍.txt","NUL:\0!").ok)
    read:=sys.read_text("data/å 🌍.txt");assert(read.ok&&read.error==0)
    assert(read.value=="Hej åäö 🌍\nNUL:\0!")
    info:=sys.metadata("data/å 🌍.txt");assert(info.ok&&info.is_file&&!info.is_dir)
    assert(info.size==i64(read.value.len))
    assert(sys.rename("data/å 🌍.txt","data/renamed.txt").ok)
    assert(!sys.metadata("data/å 🌍.txt").ok)
    assert(sys.write_text("data/renamed.txt","short").ok)
    assert(sys.read_text("data/renamed.txt").value=="short")
    print(sys.read_text("data/renamed.txt").value)
    assert(sys.remove_file("data/renamed.txt").ok)
    assert(sys.remove_dir("data").ok)
}
"#,
    );
    success(&out);
    assert_eq!(out.stdout, b"short\n");
    assert!(!work.0.join("data").exists());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn filesystem_errors_are_values_and_invalid_paths_do_not_truncate_names() {
    let work = Workspace::new();
    let out=work.run(r#"
import sys
fn main(){
    missing:=sys.read_text("missing");assert(!missing.ok&&missing.error==2&&missing.value=="")
    info:=sys.metadata("missing");assert(!info.ok&&info.error==2&&info.size==0&&!info.is_file&&!info.is_dir)
    assert(sys.mkdir("dir").ok);assert(sys.mkdir("dir").error==17)
    assert(sys.read_text("dir").error==21)
    assert(!sys.write_text("dir","text").ok)
    assert(sys.write_text("kept","original").ok)
    assert(sys.write_text("kept\0suffix","bad").error==22)
    assert(sys.rename("kept","kept\0bad").error==22)
    assert(sys.read_text("kept").value=="original")
    assert(sys.write_text("dir/file","x").ok)
    assert(!sys.remove_dir("dir").ok)
    assert(sys.remove_file("dir/file").ok);assert(sys.remove_dir("dir").ok)
    assert(sys.remove_file("missing").error==2)
    assert(sys.rename("missing","target").error==2)
    assert(sys.read_text("").error==2)
    print(sys.read_text("kept").value)
}
"#);
    success(&out);
    assert_eq!(out.stdout, b"original\n");
    assert_eq!(fs::read(work.0.join("kept")).unwrap(), b"original");
    assert!(!work.0.join("target").exists());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn read_text_validates_utf8_without_rejecting_nul_or_valid_unicode() {
    let work = Workspace::new();
    let invalid: Vec<&[u8]> = vec![
        b"\xff",
        b"\xc0\xaf",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xe2\x82",
        b"a\x80b",
        b"\xf0\x9f\x8c",
    ];
    for (i, bytes) in invalid.iter().enumerate() {
        fs::write(work.0.join(format!("bad{i}")), bytes).unwrap();
    }
    let valid = [
        "",
        "åäö 🌍",
        "日本語 العربية e\u{301} 👩‍💻",
        "a\0b",
        "\u{10ffff}",
    ];
    for (i, text) in valid.iter().enumerate() {
        fs::write(work.0.join(format!("good{i}")), text).unwrap();
    }
    let out = work.run(
        r#"
import sys
fn main(){
    for i in 0..7{r:=sys.read_text("bad"+i.str());assert(!r.ok&&r.error==92&&r.value=="")}
    for i in 0..5{r:=sys.read_text("good"+i.str());assert(r.ok);print(r.value.len)}
    assert(sys.read_text("good3").value=="a\0b")
    assert(sys.read_text("good4").value.runes()[0]==`\u{10ffff}`)
}
"#,
    );
    success(&out);
    let expected = valid
        .iter()
        .map(|s| format!("{}\n", s.len()))
        .collect::<String>();
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn reads_cover_short_empty_large_and_oversized_files_and_allocation_failure() {
    let work = Workspace::new();
    fs::write(work.0.join("large"), vec![b'x'; 1024 * 1024]).unwrap();
    let oversized = fs::File::create(work.0.join("oversized")).unwrap();
    oversized.set_len(5 * 1024 * 1024 * 1024).unwrap();
    drop(oversized);
    let out=work.run(r#"
import sys
fn main(){
    large:=sys.read_text("large");assert(large.ok&&large.value.len==1048576)
    assert(large.value[0]==120&&large.value[1048575]==120)
    meta:=sys.metadata("oversized");assert(meta.ok&&meta.size==i64(5368709120))
    assert(sys.read_text("oversized").error==27)
    assert(sys.write_text("empty","").ok);assert(sys.read_text("empty").ok)
    assert(sys.read_text("empty").value=="")
    var failed:=false
    for i in 0..20{r:=sys.read_text("large");if !r.ok{assert(r.error==12&&r.value=="");failed=true;break}}
    assert(failed);print(42)
}
"#);
    success(&out);
    assert_eq!(out.stdout, b"42\n");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn success_and_error_paths_close_handles_and_symlink_removal_preserves_the_target() {
    use std::os::unix::fs::symlink;
    let work = Workspace::new();
    work.write("valid", "å🌍");
    fs::write(work.0.join("invalid"), b"\xff").unwrap();
    symlink("valid", work.0.join("link")).unwrap();
    let out = work.run(
        r#"
import sys
fn handles()->i32{
    var total:=0
    for fd in 0..128{if sys.metadata("/dev/fd/"+fd.str()).ok{total=total+1}}
    total
}
fn main(){
    before:=handles();assert(before>0&&before<64)
    for i in 0..300 {
        assert(sys.read_text("valid").ok)
        assert(sys.read_text("invalid").error==92)
        assert(sys.read_text(".").error==21)
        assert(sys.read_text("/dev/null").error==45)
        assert(sys.read_text("missing").error==2)
    }
    assert(handles()==before)
    assert(sys.metadata("link").is_file)
    assert(sys.read_text("link").value=="å🌍")
    assert(sys.remove_file("link").ok)
    assert(sys.read_text("valid").value=="å🌍")
    var long:="";for i in 0..512{long=long+"å"}
    assert(sys.mkdir(long).error==63)
    assert(sys.metadata(long).error==63)
    print(42)
}
"#,
    );
    success(&out);
    assert_eq!(out.stdout, b"42\n");
    assert!(!work.0.join("link").exists());
    assert!(work.0.join("valid").is_file());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn sys_is_embedded_reproducible_and_runs_after_sources_are_removed() {
    let work = Workspace::new();
    work.write("main.ddr", "import sys\nfn main(){r:=sys.read_text(\"input\");assert(r.ok);print(r.value);assert(sys.write_text(\"output\",r.value).ok)}");
    work.write("input", "från fil 🌍");
    let project = work.checked();
    let image = project.compile().unwrap();
    assert_eq!(project.compile().unwrap(), image);
    let other = Workspace::new();
    other.write(
        "main.ddr",
        &fs::read_to_string(work.0.join("main.ddr")).unwrap(),
    );
    assert_eq!(other.checked().compile().unwrap(), image);
    let built = work.cli(&["build", "main.ddr", "-o", "program"]);
    success(&built);
    assert_eq!(fs::read(work.0.join("program")).unwrap(), image);
    let signed = Command::new("/usr/bin/codesign")
        .arg("--verify")
        .arg(work.0.join("program"))
        .output()
        .unwrap();
    success(&signed);
    fs::remove_file(work.0.join("main.ddr")).unwrap();
    let run = Command::new(work.0.join("program"))
        .current_dir(&work.0)
        .env_clear()
        .output()
        .unwrap();
    success(&run);
    assert_eq!(run.stdout, "från fil 🌍\n".as_bytes());
    assert_eq!(
        fs::read(work.0.join("output")).unwrap(),
        "från fil 🌍".as_bytes()
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn byte_io_utf8_decoding_and_program_arguments_preserve_their_contracts() {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let work = Workspace::new();
    work.write("untouched", "previous contents");
    work.write(
        "main.ddr",
        r#"
import sys
fn main(){
 assert(sys.arg_count()==3);assert(sys.arg(1)=="å 🌍");assert(sys.arg(2)=="")
 data:[]i32=[0,127,128,255,10]
 assert(sys.write_bytes("binary",&data).ok)
 assert(sys.make_executable("binary").ok)
 invalid:[]i32=[0,256]
 rejected:=sys.write_bytes("untouched",&invalid)
 assert(!rejected.ok&&rejected.error==22)
 valid:[]i32=[195,165,0]
 decoded:=sys.decode_utf8(&valid);assert(decoded.ok);assert(decoded.value=="å\0")
 bad:[]i32=[255]
 utf8:=sys.decode_utf8(&bad);assert(!utf8.ok&&utf8.error==92&&utf8.value=="")
 out_of_range:=sys.decode_utf8(&invalid);assert(!out_of_range.ok&&out_of_range.error==22)
 print("bytes ok")
}
"#,
    );
    let out = work.cli(&["run", "main.ddr", "--", "å 🌍", ""]);
    success(&out);
    assert_eq!(out.stdout, b"bytes ok\n");
    assert_eq!(
        fs::read(work.0.join("binary")).unwrap(),
        [0, 127, 128, 255, 10]
    );
    assert_eq!(
        fs::read_to_string(work.0.join("untouched")).unwrap(),
        "previous contents"
    );
    assert_eq!(
        fs::metadata(work.0.join("binary"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    work.write("args.ddr", "import sys\nfn main(){print(sys.arg(1))}");
    success(&work.cli(&["build", "args.ddr", "-o", "args"]));
    let invalid_arg = Command::new(work.0.join("args"))
        .arg(std::ffi::OsStr::from_bytes(&[255]))
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(invalid_arg.status.code(), Some(106));
    let missing_arg = Command::new(work.0.join("args"))
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(missing_arg.status.code(), Some(103));
}
#[test]
fn bundled_standard_modules_deduplicate_nominal_types_and_allow_explicit_local_paths() {
    let work = Workspace::new();
    fs::create_dir(work.0.join("std")).unwrap();
    work.write("std/map.ddr", "pub fn local()->i32{42}");
    work.write(
        "main.ddr",
        r#"
import "std/map" as map
import "std/map" as other
import "./std/map" as local
import sys
import "std/sys" as os
fn length(value:&other.Map)->i32{map.length(&value)}
fn main(){var m:=map.new();print(length(&m));print(local.local());print(sys.pid()==os.pid())}
"#,
    );
    let checked = work.checked();
    assert_eq!(checked.sources.files.len(), 5);
    for source in [
        "import sys\nfn main(){var x:[]i64=[];r:=sys.write_bytes(\"a\",&x)}",
        "fn main(){var x:[]i32=[];r:=__sys_decode_utf8(&x)}",
    ] {
        work.write("main.ddr", source);
        assert!(dunder::modules::check_path(work.0.join("main.ddr")).is_err());
    }
}
