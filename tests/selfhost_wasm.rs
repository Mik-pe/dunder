// The compiler built by a self-built native compiler emits the module consumed by Node.
#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
#[test]
fn selfbuilt_wasm_compiler_executes_typed_imports_and_numeric_boundaries() {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };
    struct Work(PathBuf);
    impl Drop for Work {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let work =
        Work(std::env::temp_dir().join(format!("dunder-selfhost-wasm-{}", std::process::id())));
    fs::create_dir(&work.0).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let native = root.join(if cfg!(target_os = "macos") {
        "selfhost/main.ddr"
    } else {
        "selfhost/linux.ddr"
    });
    let build = |compiler: &Path, input: &Path, output: &str| {
        Command::new(compiler)
            .current_dir(root)
            .arg("build")
            .arg(input)
            .arg("-o")
            .arg(work.0.join(output))
            .output()
            .unwrap()
    };
    let input = work.0.join("input.ddr");
    fs::write(
        &input,
        r#"
capability unsafe
@extern("host","inspect") unsafe fn inspect(p:ptr, n:i32, scale:f32, wide:f64)->i32
fn main()->i32 {
    assert((-0.49).round().is_sign_negative())
    assert((-2.5).round()==-3.0)
    assert(0.49999999999999994.round()==0.0)
    assert((-5.2).sqrt().is_nan())
    unsafe {
        f := inspect
        return f(cstr("Hello"),5, f32(2.25).sqrt(), f64.from_bits(1))
    }
}
"#,
    )
    .unwrap();
    for (compiler, source, output) in [
        (
            PathBuf::from(env!("CARGO_BIN_EXE_dunder")),
            native.clone(),
            "stage1",
        ),
        (work.0.join("stage1"), native, "stage2"),
        (
            work.0.join("stage2"),
            root.join("selfhost/wasm.ddr"),
            "wasm-compiler",
        ),
        (work.0.join("wasm-compiler"), input.clone(), "module.wasm"),
    ] {
        let out = build(&compiler, &source, output);
        assert!(out.status.success(), "{out:?}");
    }
    let script = r#"
const fs=require('node:fs'), assert=require('node:assert/strict');
const mod=new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const {exports}=new WebAssembly.Instance(mod,{host:{inspect(p,n,f,d){
    assert.equal(new TextDecoder().decode(new Uint8Array(exports.memory.buffer,p,n)),'Hello');
    assert.equal(f,1.5);assert.equal(d,Number.MIN_VALUE);return 73;
}}}); assert.equal(exports.main(),73);
"#;
    match Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(work.0.join("module.wasm"))
        .output()
    {
        Ok(o) => assert!(o.status.success(), "{o:?}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Node unavailable; skipped executing the emitted module")
        }
        Err(e) => panic!("{e}"),
    }
    fs::write(&input, "fn main(){x:=[1,2];print(x[1])}").unwrap();
    let out = build(&work.0.join("wasm-compiler"), &input, "rejected.wasm");
    assert!(!out.status.success());
    assert!(!work.0.join("rejected.wasm").exists());
}
