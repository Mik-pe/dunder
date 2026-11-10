use std::{fs, os::unix::fs::PermissionsExt, process::Command};

/// A system C compiler builds only the independent ABI oracle. Dunder emits
/// the executable and its library imports without invoking any toolchain.
#[test]
fn c_abi_roundtrips_scalars_float_aggregates_and_nested_callbacks() {
    let work = std::env::temp_dir().join(format!("dunder-c-abi-{}", std::process::id()));
    fs::create_dir_all(&work).unwrap();
    let library = work.join(if cfg!(target_os = "macos") {
        "probe.dylib"
    } else {
        "probe.so"
    });
    let c = work.join("probe.c");
    fs::write(
        &c,
        r#"
#include <stdint.h>
typedef struct { float a, b, c; } Triple;
typedef struct { double a, b; } Pair;
typedef struct { Pair origin, size; } Rect;
Triple triple(Triple v) { return (Triple){v.b, v.c, v.a}; }
Rect rect(Rect a, double step, Rect b) {
    return (Rect){{a.origin.b + step, b.origin.a}, {a.size.b, b.size.a}};
}
typedef int32_t (*Callback)(int32_t, double, int32_t, int32_t, int32_t, int32_t, int32_t, int32_t, uint16_t, int32_t);
int32_t invoke(Callback f) {
    return f(-1, 2.5, -3, 4, 5, 6, 7, 8, 65000, -10) + f(-1, 2.5, -3, 4, 5, 6, 7, 8, 65000, -10);
}
double mixed(int64_t a, double f, int32_t b, float g, uint8_t c, int64_t d,
             int32_t e, int64_t h, int32_t i, int64_t j, uint8_t k, uint16_t l,
             float m, double n, float o, double p, float q, double r,
             float s, double t, uint32_t u) {
    return a == -1 && f == 2.5 && b == -3 && g == 4.5 && c == 250 && d == 6
        && e == -7 && h == 8 && i == -9 && j == 10 && k == 244 && l == 65000
        && m == 13.5 && n == 14.5 && o == 15.5 && p == 16.5 && q == 17.5
        && r == 18.5 && s == 19.5 && t == 20.5 && u == 4000000000u ? 42.25 : 0;
}
"#,
    )
    .unwrap();
    let compiled = Command::new("cc")
        .arg(if cfg!(target_os = "macos") {
            "-dynamiclib"
        } else {
            "-shared"
        })
        .args(["-fPIC", "-o"])
        .arg(&library)
        .arg(&c)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let source = format!(
        r#"
capability unsafe
@extern("c", "abs") unsafe fn abs(x: i32) -> i32
@extern("{0}", "mixed") unsafe fn mixed(a: i64, f: f64, b: i32, g: f32, c: u8, d: i64, e: i32, h: i64, i: i32, j: i64, k: u8, l: u16, m: f32, n: f64, o: f32, p: f64, q: f32, r: f64, s: f32, t: f64, u: u32) -> f64
@c_layout struct Triple {{ a: f32, b: f32, c: f32 }}
@c_layout struct Pair {{ a: f64, b: f64 }}
@c_layout struct Rect {{ origin: Pair, size: Pair }}
@extern("{0}", "triple") unsafe fn rotate(v: Triple) -> Triple
@extern("{0}", "rect") unsafe fn rect(x: Rect, step: f64, y: Rect) -> Rect
@extern("{0}", "invoke") unsafe fn invoke(f: extern fn(i32, f64, i32, i32, i32, i32, i32, i32, u16, i32) -> i32) -> i32
unsafe fn observe(a: i32, x: f64, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: u16, i: i32) -> i32 {{
    assert(x == 2.5)
    assert(h == 65000)
    assert(a + b + c + d + e + f + g + i == 16)
    s := "callback" + b.str()
    assert(s.len == 10)
    return abs(i)
}}
@extern("/missing/unreachable", "never") unsafe fn never()
fn main() {{
    unsafe {{
        assert(abs(-8) == 8)
        f := mixed
        result := f(-1, 2.5, -3, 4.5, 250, 6, -7, 8, -9, 10, 244, 65000,
                    13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5, 20.5, 4000000000)
        assert(result == 42.25)
        v := rotate(Triple {{ a: 1.5, b: 2.5, c: 3.5 }})
        assert(v.a == 2.5 && v.b == 3.5 && v.c == 1.5)
        x := Rect {{ origin: Pair {{ a: 1.0, b: 2.0 }}, size: Pair {{ a: 3.0, b: 4.0 }} }}
        y := Rect {{ origin: Pair {{ a: 5.0, b: 6.0 }}, size: Pair {{ a: 7.0, b: 8.0 }} }}
        out := rect(x, 0.25, y)
        assert(out.origin.a == 2.25 && out.origin.b == 5.0 && out.size.a == 4.0 && out.size.b == 7.0)
        assert(invoke(callback(observe)) == 20)
    }}
}}
"#,
        library.display()
    );
    let image =
        dunder::compile(&source).unwrap_or_else(|e| panic!("{}", e.render("ffi.ddr", &source)));
    let binary = work.join("probe");
    fs::write(&binary, image).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(binary).env_clear().output().unwrap();
    let _ = fs::remove_dir_all(work);
    assert!(output.status.success(), "{output:?}");
}
