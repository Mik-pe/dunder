use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn native(source: &str) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!(
        "dunder-collections-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("main.ddr"), source).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_dunder"))
        .args(["run", "main.ddr"])
        .env("PATH", "/no-external-tools")
        .current_dir(&dir)
        .output()
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
    result
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
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn string_sort_matches_rust_order_for_generated_utf8_keys() {
    let pieces = ["", "a", "z", "å", "ä", "🌍", "\0", "\n", "a,b", "\""];
    let mut state = 1234567_u64;
    let mut values = Vec::new();
    for i in 0..2048 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let left = pieces[(state >> 32) as usize % pieces.len()];
        let right = pieces[(state >> 16) as usize % pieces.len()];
        values.push(format!("{left}{}{right}", i % 97));
    }
    let mut source = String::from("import std.array\nfn main(){var keys:[]string=[]\n");
    for value in &values {
        source.push_str(&format!("keys.push({value:?})\n"));
    }
    source.push_str("original:=keys\narray.sort_strings(&mut keys)\n");
    source.push_str(&format!("assert(original[0]=={:?})\n", values[0]));
    values.sort();
    for (i, value) in values.iter().enumerate() {
        source.push_str(&format!("assert(keys[{i}]=={value:?})\n"));
    }
    source.push_str("print(\"sorted\")}\n");
    let result = native(&source);
    success(&result);
    assert_eq!(result.stdout, b"sorted\n");
}
#[test]
fn borrows_and_dynamic_mutation_obey_types_and_immutability() {
    for source in [
        "fn main(){a:[]i32=[];a.push(1)}",
        "fn main(){var a:[]i32=[];a.push(true)}",
        "fn f(a:&[]i32){a.push(1)} fn main(){}",
        "fn f(a:&mut i32,b:&i32){} fn main(){var x:=1;f(&mut x,&x)}",
        "fn main(){var x:=1;y:=&mut x}",
        "fn main(){var x:=1;y:&mut i32=&mut x}",
        "fn f()->&i32{var x:=1;return &x} fn main(){}",
        "struct Bad{value:&i32} fn main(){}",
        "enum Bad{Value(&i32)} fn main(){}",
        "fn f(a:&mut i32){} fn main(){x:=1;f(&mut x)}",
        "fn f(a:&mut i32){} fn main(){var x:=1;f(&x)}",
        "fn main(){var a:[]i32=[];a.push(1);a[0]=false}",
        "fn grow(a:&mut []i32)->i32{a.reserve(99);1} fn main(){var a:[]i32=[1];a[0]=grow(&mut a)}",
        "fn grow(a:&mut []i32)->i32{a.reserve(99);1} fn sink(a:&mut i32,x:i32){a=x} fn main(){var a:[]i32=[1];sink(&mut a[0],grow(&mut a))}",
    ] {
        assert!(dunder::check(source).is_err(), "accepted {source}");
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
fn arrays_grow_clone_and_borrow_without_losing_value_semantics() {
    let o = native(
        r#"
fn append(a:&mut []i32,value:i32){a.push(value)}
fn sum(a:&[]i32)->i32{var total:=0;var i:=0;while i<a.len{total=total+a[i];i=i+1};total}
fn increment(x:&mut i32){x=x+1}
fn main(){
 var a:[]i32=[];for i in 0..10000{append(&mut a,i)}
 assert(a.len==10000);assert(sum(&a)==49995000);assert(a.cap()>=10000)
 copy:=a;a[0]=-10;assert(copy[0]==0)
 a.reserve(32768);assert(a.cap()>=32768);assert(a[0]==-10)
 last:=a.pop();assert(last==9999);assert(a.len==9999)
 var x: i32=-1;increment(&mut x);assert(x==0);increment(&mut x);assert(x==1)
 for item in a{if item==-10{a.clear()}}
 assert(a.len==0);assert(copy.len==10000)
 var text:[]string=["å","🌍"];text.push("tail");old:=text;text[0]="new";assert(old[0]=="å");assert(text.pop()=="tail")
 print(sum(&copy));print(old[1])
}
"#,
    );
    success(&o);
    assert_eq!(o.stdout, "49995000\n🌍\n".as_bytes());
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn arrays_in_records_enums_callbacks_and_returns_are_deep_copies() {
    let o = native(
        r#"
struct Table{keys:[]string,values:[]i64,count:i32}
enum Value{None,Items([]i32),Table(Table)}
fn make()->Table{Table{keys:["a","b"],values:[1,2],count:2}}
fn invoke(f:fn()->Table)->Table{f()}
fn update(t:&mut Table){t.keys[0]="changed";t.values.push(3);t.count=3}
fn main(){
 var a:=invoke(make);snapshot:=a;update(&mut a)
 assert(snapshot.keys[0]=="a");assert(snapshot.values.len==2);assert(a.values.len==3);assert(a.count==3)
 var v:=Value.Table(a);copy:=v;v=.None
 match copy{.Table(t)=>{assert(t.keys[0]=="changed");assert(t.values[2]==3)},_=>{assert(false)}}
 var nums:[]i32=[1,2];items:=Value.Items(nums);nums[0]=9
 match items{.Items(values)=>{assert(values[0]==1)},_=>{assert(false)}}
 print(snapshot.keys[0]);print(a.count)
}
"#,
    );
    success(&o);
    assert_eq!(o.stdout, b"a\n3\n");
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn dynamic_bounds_empty_pop_and_excess_capacity_have_defined_failures() {
    for (source, status) in [
        ("fn main(){var a:[]i32=[];print(a[0])}", 103),
        ("fn main(){var a:[]i32=[];x:=a.pop()}", 103),
        ("fn main(){var a:[]i32=[];a.reserve(-1)}", 109),
        ("fn main(){var a:[]i64=[];a.reserve(2147483647)}", 109),
    ] {
        let o = native(source);
        assert_eq!(
            o.status.code(),
            Some(status),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
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
fn dynamic_scalar_layouts_nested_aggregates_and_call_snapshots_are_consistent() {
    let o = native(
        r#"
enum Kind{A=-1,B=7}
struct Inner{items:[]i32,wide:f64}
struct Outer{inner:Inner,fixed:[i64;2]}
fn take_before(first:[]i32,second:i32)->i32{first[0]}
fn mutate(a:&mut []i32)->i32{a[0]=99;0}
fn main(){
 var flags:[]bool=[true,false];flags.push(true);assert(flags.pop());assert(!flags[1])
 var small:[]f32=[1.25,2.5];small.push(4.0);assert(small.pop()==4.0);assert(small[0]+small[1]==3.75)
 var wide:[]f64=[1.0e200];wide.push(2.5);assert(wide[0]==1.0e200);assert(wide.pop()==2.5)
 var kinds:[]Kind=[.A];kinds.push(.B);assert(kinds.pop()==.B);assert(i32(kinds[0])==-1)
 var outer:=Outer{inner:Inner{items:[1,2],wide:2.0},fixed:[9,10]}
 snapshot:=outer;outer.inner.items.push(3);outer.fixed[0]=99
 assert(snapshot.inner.items.len==2);assert(snapshot.fixed[0]==9);assert(outer.inner.items[2]==3)
 var data:[]i32=[7]
 result:=take_before(data,mutate(&mut data));assert(result==7);assert(data[0]==99)
 value:i64=-1;assert(value.shr(63)==1);assert(value.bit_not()==0)
 assert(value.wrapping_add(1)==0);assert(value.bit_and(255)==255)
 narrow:i32=2147483647;assert(narrow.wrapping_add(1)==-2147483648)
 assert(1.shl(7)==128)
 print("layouts ok")
}
"#,
    );
    success(&o);
    assert_eq!(o.stdout, b"layouts ok\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn map_and_set_match_independent_host_operations_in_a_large_native_image() {
    use std::collections::{HashMap, HashSet};
    let mut map = HashMap::<String, i64>::new();
    let mut set = HashSet::<String>::new();
    let mut keys = vec![
        "".to_owned(),
        "\0".to_owned(),
        "å".to_owned(),
        "a\u{30a}".to_owned(),
        "🌍".to_owned(),
        "a".to_owned(),
        "i".to_owned(),
        "q".to_owned(),
        "y".to_owned(),
    ];
    keys.extend((0..71).map(|i| format!("key-{i}")));
    let mut source=String::from("import \"std/map\" as map\nimport \"std/set\" as set\nimport sys\nfn main(){\nvar m:=map.new();var s:=set.new()\n");
    let mut seed = 123u32;
    for i in 0..6000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let key = &keys[(seed as usize / 4) % keys.len()];
        let literal = format!("{key:?}");
        let value = i64::from(seed % 10001) - 5000;
        match seed % 4 {
            0 => {
                map.insert(key.clone(), value);
                let fresh = set.insert(key.clone());
                source+=&format!("map.put(&mut m,{literal},i64({value}));assert(set.insert(&mut s,{literal})=={fresh})\n");
            }
            1 => {
                let removed = map.remove(key).is_some();
                let removed_set = set.remove(key);
                source+=&format!("assert(map.remove(&mut m,{literal})=={removed});assert(set.remove(&mut s,{literal})=={removed_set})\n");
            }
            2 => {
                let found = map.contains_key(key);
                let found_set = set.contains(key);
                source+=&format!("assert(map.contains(&m,{literal})=={found});assert(set.contains(&s,{literal})=={found_set})\n");
            }
            _ => {
                let value = map.get(key).copied().unwrap_or(-999999);
                source += &format!("assert(map.get_or(&m,{literal},-999999)=={value})\n");
            }
        }
        if i % 101 == 0 {
            source += &format!(
                "assert(map.length(&m)=={});assert(set.length(&s)=={})\n",
                map.len(),
                set.len()
            );
        }
    }
    let sum: i64 = map.values().sum();
    source+="var sum:i64=0\nfor key in map.keys(&m){sum=sum+map.get_or(&m,key,0)}\nprint(sum);print(set.length(&s));print(\"reference ok\")\nassert(sys.metadata(sys.arg(0)).size>1048576)\n}\n";
    let out = native(&source);
    success(&out);
    assert_eq!(
        out.stdout,
        format!("{sum}\n{}\nreference ok\n", set.len()).as_bytes()
    );
    // The first assertion branches over >1 MiB of code to a distant runtime trap.
    let failing = source.replacen("fn main(){", "fn main(){assert(false);", 1);
    let out = native(&failing);
    assert_eq!(out.status.code(), Some(104));
}

#[test]
fn unreachable_functions_and_float_formatting_do_not_change_the_image() {
    let simple = dunder::compile("fn main(){print(42)}").unwrap();
    let unused =
        dunder::compile("fn unused(){print(0.1);print(\"unused sentinel\")} fn main(){print(42)}")
            .unwrap();
    assert_eq!(simple, unused);
    assert!(dunder::check("fn unused(){x:=true+1} fn main(){print(42)}").is_err());
}
