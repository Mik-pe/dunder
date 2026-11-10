//! Optional cross-target verifier; QEMU is a test dependency only.
//! Run from the repository root:
//! cargo build
//! rustc --edition 2021 tools/verify-arm64.rs --extern dunder=target/debug/libdunder.rlib -L dependency=target/debug/deps -o target/verify-arm64
//! target/verify-arm64 /path/to/qemu-aarch64
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path, process::Command};
fn main() {
    let qemu = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "qemu-aarch64".into());
    let root = std::env::current_dir().unwrap();
    let scratch =
        Scratch(std::env::temp_dir().join(format!("dunder-arm64-verify-{}", std::process::id())));
    fs::create_dir(&scratch.0).unwrap();
    let out = &scratch.0;
    for (source, expected) in [
        ("examples/hello.ddr", "42\ntrue\n"),
        ("examples/fibonacci.ddr", "6765\ntrue\n"),
        ("examples/functions.ddr", "42\n42\n-42\n"),
        ("examples/modules/main.ddr", "42\ntrue\n42\n"),
        ("dogfood/collections.ddr", "-2\n-1\n0\n1\n13\n3\n"),
        ("dogfood/recursion.ddr", "6765\n21\n"),
        ("dogfood/sorting.ddr", "45\n9\n"),
        ("dogfood/iterators.ddr", "36\n20\n0\n"),
        ("dogfood/arena/main.ddr", "28\n"),
        ("dogfood/structs.ddr", "25\n32\n3\n"),
        ("dogfood/numeric/main.ddr", "5.0\n2.0\ntrue\n"),
        ("dogfood/filesystem.ddr", "18\nfilesystem ok\n"),
        ("dogfood/enums.ddr", "42\nenums ok\n"),
        ("dogfood/standard.ddr", "4\n6\nstdlib ok\n"),
        (
            "dogfood/utf8.ddr",
            "bytes=25\nrunes=20\nwords=5\n🌍 Hej världen\n",
        ),
        (
            "dogfood/tokenizer.ddr",
            "fn|hälsa|(|namn|)|{|print|(|\"Hej 🌍\"|)|}\n",
        ),
    ] {
        let checked = dunder::modules::load(&root.join(source))
            .unwrap_or_else(|e| panic!("{}", e.render()))
            .check()
            .unwrap_or_else(|e| panic!("{}", e.render()));
        let bin = out.join(source.replace('/', "-"));
        let image = checked
            .compile_for(dunder::target::Target::Arm64Linux)
            .unwrap();
        fs::write(&bin, image).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let result = execute(&qemu, &bin, out, &[]);
        assert!(
            result.status.success(),
            "{}: {:?} {}",
            source,
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout),
            expected,
            "{}",
            source
        );
        println!("PASS {source}");
    }
    let checked = dunder::modules::load(&root.join("dogfood/tests.ddr"))
        .unwrap_or_else(|e| panic!("{}", e.render()))
        .check_tests()
        .unwrap_or_else(|e| panic!("{}", e.render()));
    for (i, test) in checked.program.tests.iter().enumerate() {
        let bin = out.join(format!("test-{i}"));
        let image = dunder::native::compile_entry_for(
            &checked.program,
            test.function,
            dunder::target::Target::Arm64Linux,
        )
        .unwrap();
        fs::write(&bin, image).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let result = execute(&qemu, &bin, out, &[]);
        assert!(
            result.status.success(),
            "test {}: {:?} {}",
            test.name,
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
        println!("PASS test {}", test.name);
    }
    edges(&qemu, out);
    println!("16 examples/dogfood, {} test groups, and platform edge cases passed on ARM64 Linux under QEMU", checked.program.tests.len());
}

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn execute(
    qemu: &std::ffi::OsStr,
    bin: &Path,
    cwd: &Path,
    args: &[&std::ffi::OsStr],
) -> std::process::Output {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let mut child = Command::new(qemu)
        .arg(bin)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("ARM64 verification timed out: {}", bin.display());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn build(out: &Path, source: &str) -> std::path::PathBuf {
    let path = out.join("edge.ddr");
    fs::write(&path, source).unwrap();
    let checked = dunder::modules::load(&path)
        .unwrap_or_else(|e| panic!("{}", e.render()))
        .check()
        .unwrap_or_else(|e| panic!("{}", e.render()));
    let bin = out.join("edge");
    fs::write(
        &bin,
        checked
            .compile_for(dunder::target::Target::Arm64Linux)
            .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn edges(qemu: &std::ffi::OsStr, out: &Path) {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    fs::write(out.join("original"), b"hello").unwrap();
    fs::hard_link(out.join("original"), out.join("hardlink")).unwrap();
    symlink("original", out.join("symlink")).unwrap();
    symlink("loop", out.join("loop")).unwrap();
    fs::create_dir(out.join("nonempty")).unwrap();
    fs::write(out.join("nonempty/child"), b"x").unwrap();
    let bin = build(
        out,
        r#"
import sys
fn main(){
    assert(sys.arg_count()==3);assert(sys.arg(1)=="å 🌍");assert(sys.arg(2)=="")
    assert(sys.pid()>0)
    assert(sys.same_file("original","hardlink"));assert(sys.same_file("symlink","original"))
    assert(!sys.same_file("missing","missing"));assert(!sys.same_file("original\0tail","original"))
    info:=sys.metadata("symlink");assert(info.ok&&info.is_file&&!info.is_dir&&info.size==5)
    assert(sys.metadata("nonempty").is_dir)
    assert(sys.remove_dir("nonempty").error==66)
    assert(sys.metadata("loop").error==62)
    assert(sys.metadata("original/child").error==20)
    var long:="";for i in 0..300{long=long+"a"}
    assert(sys.mkdir(long).error==63);assert(sys.metadata(long).error==63)
    assert(sys.metadata("missing").error==2)
    data:[]i32=[0,127,128,255,10]
    assert(sys.write_bytes("binary",&data).ok);assert(sys.make_executable("binary").ok)
    invalid:[]i32=[0,256]
    assert(sys.write_bytes("original",&invalid).error==22)
    assert(sys.read_text("original").value=="hello")
    bad:[]i32=[255];assert(sys.decode_utf8(&bad).error==92)
    assert(sys.write_text("roundtrip","å\0🌍").ok)
    assert(sys.append_text("roundtrip","!").ok)
    assert(sys.read_text("roundtrip").value=="å\0🌍!")
    assert(sys.rename("roundtrip","renamed").ok);assert(sys.remove_file("renamed").ok)
    print("platform edges ok")
}
"#,
    );
    let result = execute(
        qemu,
        &bin,
        out,
        &[std::ffi::OsStr::new("å 🌍"), std::ffi::OsStr::new("")],
    );
    assert!(result.status.success(), "platform edges: {:?}", result);
    assert_eq!(result.stdout, b"platform edges ok\n");
    assert_eq!(
        fs::read(out.join("binary")).unwrap(),
        [0, 127, 128, 255, 10]
    );
    assert_eq!(
        fs::metadata(out.join("binary"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    let bin = build(out, "import sys\nfn main(){print(sys.arg(1))}");
    assert_eq!(execute(qemu, &bin, out, &[]).status.code(), Some(103));
    assert_eq!(
        execute(qemu, &bin, out, &[std::ffi::OsStr::from_bytes(&[255])])
            .status
            .code(),
        Some(106)
    );
    let bin = build(
        out,
        r#"
fn main(){
    assert(f64.from_bits(1)>0.0);assert(f64.from_bits(1)/2.0==0.0)
    assert(f32.from_bits(1)>0.0);assert(f32.from_bits(1)*2.0==f32.from_bits(2))
    assert((-0.0).is_sign_negative());assert(!(-0.0).abs().is_sign_negative())
    assert(2.5.round()==3.0);assert((-2.5).round()==-3.0)
    assert(2.5.round_ties_even()==2.0);assert(3.5.round_ties_even()==4.0)
    assert((-1.2).floor()==-2.0);assert((-1.2).ceil()==-1.0)
    inf:=1.0/0.0;nan:=0.0/0.0
    assert(inf.is_infinite());assert(nan.is_nan());assert(nan!=nan)
    assert(!(nan<0.0));assert(!(nan>=0.0));assert((-1.0).sqrt().is_nan())
    assert(i64(-9223372036854775808.0)==i64(-9223372036854775808))
    print("floating edges ok")
}
"#,
    );
    let result = execute(qemu, &bin, out, &[]);
    assert!(result.status.success(), "float edges: {:?}", result);
    assert_eq!(result.stdout, b"floating edges ok\n");
    for (source, code) in [
        ("fn main(){x:=2147483647;print(x+1)}", 101),
        ("fn main(){x:=0;print(1/x)}", 102),
        ("fn main(){a:=[1];print(a[1])}", 103),
        ("fn main(){assert(false)}", 104),
        ("fn main(){print(i32(1.0/0.0))}", 107),
    ] {
        let bin = build(out, source);
        assert_eq!(
            execute(qemu, &bin, out, &[]).status.code(),
            Some(code),
            "{source}"
        );
    }
    println!("PASS platform, arguments, error-status and IEEE edge cases");
}
