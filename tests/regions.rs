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

const PROGRAM: &str = r#"
struct Node { left:i32, right:i32, score:i64 }
fn twice(a:&[]i64)->i64 { return a[0]*2 }
fn adjust(a:&mut i64){a=a+7}
fn node_score()->i64 {
    region nodes(128) {
        var a:[]Node=nodes.alloc(5)
        a[4]=Node{left:1,right:2,score:1000}
        a[2].score=a[4].score+10
        copy:=a[2]
        adjust(&mut a[2].score)
        assert(copy.score==1010)
        return a[2].score
    }
}
fn length(n:&mut i32)->i32 { n=n+1; return 2 }
fn pick()->i64 {
    region scratch(32) {
        var calls:=0
        var flags:[]bool=scratch.alloc(1)
        var numbers:[]i64=scratch.alloc(length(&mut calls))
        assert(calls==1 && numbers.len==2 && numbers.len()==2 && len(numbers)==2)
        flags[0]=true
        numbers[0]=3000000000
        assert(twice(&numbers)==6000000000)
        region scratch(4) { var tiny:[]i32=scratch.alloc(1);tiny[0]=11;numbers[1]=i64(tiny[0]) }
        assert(flags[0])
        return numbers[0]+numbers[1]
    }
}
fn main() {
    var i:=0
    while i<40 {
        i=i+1
        region temp(33554432) {
            var small:[]i32=temp.alloc(3)
            assert(small[2]==0)
            small[2]=42
            if i<39 {continue}
            if i==39 {break}
        }
    }
    region empty(0) { z:[]i32=empty.alloc(0);assert(z.len==0) }
    assert(node_score()==1017)
    print(pick())
}
"#;

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
