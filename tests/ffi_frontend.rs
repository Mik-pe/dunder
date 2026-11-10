use dunder::{check, check_for, syntax::Type, target::Target};

fn error(source: &str) -> dunder::Error {
    check(source)
        .err()
        .expect("expected the checker to reject this program")
}

#[test]
fn c_layout_matches_native_and_wasm32_including_nested_struct_alignment() {
    let source = r#"
@c_layout struct Pair { x: u8, y: u16 }
@c_layout struct Packet { lead: u8, pair: Pair, buf: [u8; 3], len: usize, data: ptr, stamp: u64 }
fn main() {}
"#;
    for (target, bytes, offsets) in [
        (Target::Arm64Macos, 40, vec![0, 2, 6, 16, 24, 32]),
        (Target::X86_64Linux, 40, vec![0, 2, 6, 16, 24, 32]),
        (Target::Wasm32, 32, vec![0, 2, 6, 12, 16, 24]),
    ] {
        let program = check_for(source, target).unwrap();
        let pair = program.types.record(Type::Record(0)).unwrap();
        assert_eq!((pair.bytes, pair.alignment), (4, 2));
        let packet = program.types.record(Type::Record(1)).unwrap();
        assert_eq!((packet.bytes, packet.alignment), (bytes, 8));
        assert_eq!(
            packet.fields.iter().map(|f| f.offset).collect::<Vec<_>>(),
            offsets
        );
    }
}

#[test]
fn unsafe_function_values_require_unsafe_even_after_assignment() {
    for declaration in [
        "@extern(\"c\", \"abs\") unsafe fn external(x: i32) -> i32",
        "unsafe fn external(x: i32) -> i32 { x }",
    ] {
        let source =
            format!("capability unsafe\n{declaration}\nfn main() -> i32 {{ f := external; f(7) }}");
        assert_eq!(error(&source).code, "E0290");
        let laundering = format!(
            "capability unsafe\n{declaration}\nfn main() {{ f: fn(i32) -> i32 = external }}"
        );
        assert_eq!(error(&laundering).code, "E0201");
    }
}

#[test]
fn module_capability_and_unsafe_context_are_both_required() {
    for source in [
        "@extern(\"c\", \"abs\") unsafe fn f(x: i32) -> i32\nfn main() {}",
        "fn main() { unsafe { p := ptr(0) } }",
        "capability unsafe\nfn main() { p := ptr(0) }",
        "capability unsafe\nfn main() { unsafe { p := ptr(0) }; q := ptr(1) }",
        "capability unsafe\n@extern(\"c\", \"f\") fn f()\nfn main() {}",
    ] {
        assert_eq!(error(source).code, "E0290", "{source}");
    }
    assert!(check("capability unsafe\nfn main() { unsafe { p := ptr(0) } }").is_ok());
}

#[test]
fn validates_c_types_and_rejects_invalid_unsigned_literals() {
    for field in ["string", "[]u8", "[u8; 0]", "fn(i32) -> i32", "Normal"] {
        let source = format!("struct Normal {{ a: i32 }}\n@c_layout struct Invalid {{ field: {field} }}\nfn main() {{}}");
        assert!(check(&source).is_err(), "{field}");
    }
    for ty in ["string", "[]u8", "&i32", "fn(i32) -> i32", "Record"] {
        let source = format!("capability unsafe\n@c_layout struct Record {{ a: i32 }}\n@extern(\"c\", \"a\") unsafe fn foreign(arg: {ty})\nfn main() {{}}");
        assert_eq!(error(&source).code, "E0291", "{ty}");
    }
    assert!(check("fn main() { x: u8 = 256 }").is_err());
    assert!(check("fn main() { x: u8 = -1 }").is_err());
    assert!(check("fn main() { x: u64 = 18446744073709551615 }").is_ok());
    let wide = "fn main() { x: usize = 4294967296 }";
    assert!(check_for(wide, Target::Arm64Macos).is_ok());
    assert!(check_for(wide, Target::Wasm32).is_err());
}

#[test]
fn typed_calls_reads_writes_and_size_queries_compose() {
    let source = r#"
capability unsafe
@c_layout struct Api { f: extern fn(ptr) -> u32 }
@c_layout struct Buffer { data: [u8; 32], len: usize }
fn read(buf: &Buffer) -> usize { buf.len }
unsafe fn example(symbol: ptr) -> u32 {
    var table: Api = zeroed()
    table.f = ffi_fn(symbol)
    var buf: Buffer = zeroed()
    buf.len = size_of(Buffer)
    p := addr(&mut buf.data[0])
    store_u16(p, 513)
    assert(load_u8(p) == 1)
    assert(read(ref_at(addr(&buf))) == size_of(Buffer))
    table.f(p)
}
fn main() {}
"#;
    check(source).unwrap_or_else(|e| panic!("{}", e.render("ffi.ddr", source)));
}

#[test]
fn forward_declared_hfa_and_callback_use_checked_c_signature() {
    let source = r#"
capability unsafe
@c_layout struct Dispatch { call: extern fn(Bounds) -> Bounds }
@c_layout struct Bounds { origin: Point, extent: Point }
@c_layout struct Point { x: f64, y: f64 }
@extern("c", "round_rect") unsafe fn round_rect(b: Bounds) -> Bounds
fn positive(i: i32) -> i32 { i }
unsafe fn make() -> Dispatch { Dispatch { call: round_rect } }
unsafe fn thunk() -> extern fn(i32) -> i32 { callback(positive) }
fn main() {}
"#;
    let program = check(source).unwrap();
    assert_eq!(
        program.types.homogeneous_float(Type::Record(1)),
        Some((Type::F64, 4))
    );
}
