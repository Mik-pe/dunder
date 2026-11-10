use dunder::target::Target;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn execute(source: &str, driver: &str) {
    let image = dunder::compile_for(source, Target::Wasm32)
        .unwrap_or_else(|e| panic!("{}", e.render("fixture.ddr", source)));
    assert_eq!(image, dunder::compile_for(source, Target::Wasm32).unwrap());
    execute_image(&image, driver);
}

fn execute_image(image: &[u8], driver: &str) {
    assert_eq!(&image[..8], b"\0asm\x01\0\0\0");
    let script = format!(
        r#"const assert=require('node:assert/strict');
const module_ = new WebAssembly.Module(require('node:fs').readFileSync(0));
{driver}"#
    );
    let child = Command::new("node")
        .arg("-e")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("node unavailable; skipped wasm runtime oracle");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    child.stdin.take().unwrap().write_all(image).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn host_imports_are_typed_reachable_and_use_wasm32_layout() {
    execute(
        r#"
capability unsafe
@c_layout
struct Packet { tag: u8, pointer: ptr, payload: u64 }
@extern("host", "inspect") unsafe fn inspect(p: ptr, s: ptr, n: usize, f: extern fn(i32) -> i32) -> i32
@extern("missing", "unused") unsafe fn unused() -> i64
fn square(x:i32)->i32 { x*x }
fn main() -> i32 {
    unsafe {
        var p: Packet = zeroed()
        p.tag = 127
        p.payload = 12345678901234
        f := inspect
        return f(addr(&mut p), cstr("Hi"), size_of(Packet), callback(square))
    }
}
"#,
        r#"
assert.deepEqual(WebAssembly.Module.imports(module_),[{module:'host',name:'inspect',kind:'function'}]);
const {exports} = new WebAssembly.Instance(module_,{host:{inspect:(p,s,n,f)=>{
    const view=new DataView(exports.memory.buffer);
    assert.equal(n,16);
    assert.equal(view.getUint8(p),127);
    assert.equal(view.getUint32(p+4,true),0);
    assert.equal(view.getBigUint64(p+8,true),12345678901234n);
    assert.deepEqual([...new Uint8Array(view.buffer,s,3)],[72,105,0]);
    return exports.__indirect_function_table.get(f)(7);
}}});
assert.equal(exports.main(),49);
"#,
    );
}

#[test]
fn structured_control_memory_growth_and_integer_traps() {
    execute(
        r#"
fn alter(n:&mut i32) { n=n+1 }
fn large() -> i32 {
    var a:[i32;16384]=[7;16384]
    a[16383]=42
    return a[16383]
}
fn main() -> i32 {
    var total:=0
    for i in 0..8 {
        if i==2 { continue }
        if i==6 { break }
        total=total+i
    }
    alter(&mut total)
    assert(total==14)
    return large()+total
}
"#,
        r#"
assert.deepEqual(WebAssembly.Module.imports(module_),[]);
const {exports} = new WebAssembly.Instance(module_);
assert.equal(exports.memory.buffer.byteLength,65536);
assert.equal(exports.main(),56);
const after=exports.memory.buffer.byteLength;
assert(after>65536);
assert.equal(exports.main(),56);
assert.equal(exports.memory.buffer.byteLength,after);
"#,
    );
    for source in [
        "fn main()->i32{var a:=2147483647;return a+1}",
        "fn main()->i32{a:i64=-9223372036854775808;return i32(a % -1)}",
        "fn main()->i32{a:u8=255;return i32(a+1)}",
        "fn main()->i32{n:i32=32;return i32(1).shl(n)}",
    ] {
        execute(source,"assert.throws(()=>new WebAssembly.Instance(module_).exports.main(),WebAssembly.RuntimeError);");
    }
}

#[test]
fn floats_callbacks_and_packed_integers_use_typed_wasm_values() {
    execute(
        r#"
fn invoke(f:fn(f64)->f64,x:f64)->f64 { f(x) }
fn twice(x:f64)->f64 { x*2.0 }
fn main(){
    a:u32=4294967295
    assert(a.wrapping_add(1)==0)
    x:i64=9007199254740993
    assert(x>9007199254740992)
    assert(invoke(twice,2.25)==4.5)
    assert((-2.5).round()== -3.0)
    assert(i64(rune(128640))==128640)
    print(2.5)
}
"#,
        r#"let printed=[];new WebAssembly.Instance(module_,{dunder:{print_f64:x=>printed.push(x)}}).exports.main();assert.deepEqual(printed,[2.5]);"#,
    );
}

#[test]
fn strings_aggregate_owners_and_regions_reuse_linear_memory() {
    execute(
        r#"
struct Labels { items: []string }
fn test(seed: i32) -> Labels {
    var items: []string = []
    items.push(seed.str() + "λ")
    var value:=Labels{items:items}
    value=value
    return value
}
fn main() {
    for n in 0..2000 {
        original:=test(n)
        var copy:=original
        text:=copy.items.pop()
        assert(text==original.items[0] && copy.items.len==0)
        region scratch(80000) {
            var data:[]u16=scratch.alloc(30000)
            assert(data[29999]==0)
            data[29999]=65535
        }
    }
}
"#,
        r#"
const {exports}=new WebAssembly.Instance(module_);
exports.main();
const peak=exports.memory.buffer.byteLength;
assert(peak>=80000 && peak<1024*1024);
exports.main();
assert.equal(exports.memory.buffer.byteLength,peak);
"#,
    );
}

#[test]
fn actual_vulkan_scene_geometry_sha256_and_utf8_run_without_host_imports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (file, selected) in [
        (
            "dogfood/vulkan/scene.ddr",
            "scene_reuses_storage_and_emits_finite_triangle_geometry",
        ),
        (
            "stdlib/sha256.ddr",
            "sha256_matches_empty_abc_and_multi_block_reference_vectors",
        ),
    ] {
        let checked = dunder::modules::load(root.join(file))
            .and_then(|p| p.check_tests_for(Target::Wasm32))
            .unwrap_or_else(|e| panic!("{}", e.render()));
        let case = checked
            .program
            .tests
            .iter()
            .find(|t| t.name == selected)
            .unwrap();
        let image =
            dunder::native::compile_entry_for(&checked.program, case.function, Target::Wasm32)
                .unwrap();
        execute_image(&image, "assert.deepEqual(WebAssembly.Module.imports(module_),[]);new WebAssembly.Instance(module_).exports.main();");
    }
}
