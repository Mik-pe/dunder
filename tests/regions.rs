use dunder::target::Target;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn run(source: &str) -> Output {
    let image = dunder::compile_for(source, Target::host().unwrap()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "dunder-region-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, image).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let result = Command::new(&path).output().unwrap();
    fs::remove_file(path).unwrap();
    result
}

#[test]
fn region_views_cannot_escape_or_resize() {
    for use_view in [
        "return data",
        "out=data",
        "copied:=data",
        "data=[]",
        "data.push(1)",
        "data.reserve(40)",
        "data.clear()",
        "mutate(&mut data)",
        "other:=scratch",
        "scratch.clear()",
    ] {
        let src = format!(
            "fn mutate(a:&mut []i32){{a.clear()}} fn read(a:&i32){{}} fn f()->[]i32{{
            var out:[]i32=[]; region scratch {{ var data:[]i32=scratch.alloc(1); {use_view} }}; return out
            }} fn main(){{}}"
        );
        let error = dunder::check(&src)
            .err()
            .unwrap_or_else(|| panic!("accepted {use_view}"));
        assert_eq!(error.code, "E0282", "{use_view}: {error:?}");
    }
    let raw_escape = "capability unsafe\nunsafe fn leak()->ptr{region scratch{var s:[]i32=scratch.alloc(1);return addr(&mut s[0])}} fn main(){}";
    assert_eq!(dunder::check(raw_escape).err().unwrap().code, "E0282");
}

const PROGRAM: &str = include_str!("fixtures/regions.ddr");

#[test]
fn native_regions_align_zero_and_release_on_every_exit() {
    for target in Target::NATIVE {
        dunder::compile_for(PROGRAM, target).unwrap();
    }
    let result = run(PROGRAM);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"3000000011\n");
}

#[test]
fn region_capacity_and_slice_bounds_are_enforced() {
    for (capacity, body) in [
        ("3", "a:[]i32=s.alloc(1)"),
        ("8", "a:[]bool=s.alloc(1); b:[]i64=s.alloc(1)"),
        ("8", "a:[]i64=s.alloc(-1)"),
        ("8", "a:[]i64=s.alloc(2147483647)"),
        ("16", "a:[]i32=s.alloc(1); b:[]i32=s.alloc(1); print(a[1])"),
    ] {
        let src = format!("fn main(){{region s({capacity}){{{body}}}}}");
        let output = run(&src);
        assert_eq!(output.status.code(), Some(103), "{src} {output:?}");
    }
}
