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
            "dunder-floats-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    fn native(source: &str) -> std::process::Output {
        use std::os::unix::fs::PermissionsExt;
        let work = Self::new();
        let path = work.0.join("program");
        let image =
            dunder::compile(source).unwrap_or_else(|e| panic!("{}", e.render("float.ddr", source)));
        fs::write(&path, image).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        #[cfg(target_os = "macos")]
        {
            let signed = Command::new("/usr/bin/codesign")
                .arg("--verify")
                .arg(&path)
                .output()
                .unwrap();
            assert!(signed.status.success());
        }
        Command::new(path).env_clear().output().unwrap()
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn floating_literals_types_and_methods_are_checked() {
    for (source, code) in [
        ("fn main(){x:f32=1.0;y:f64=x}", "E0201"),
        ("fn main(){x:f64=1.0;y:f32=x}", "E0201"),
        ("fn main(){x:=1;print(x+1.5)}", "E0201"),
        ("fn main(){x:=1.5;print(x+true)}", "E0201"),
        ("fn main(){print(2.0%1.0)}", "E0206"),
        ("fn main(){if 1.0{}}", "E0201"),
        ("fn main(){print(f32(true))}", "E0201"),
        ("fn main(){print(rune(3.0))}", "E0206"),
        ("fn main(){x:f32=1e100}", "E0260"),
        ("fn main(){x:=1e9999}", "E0260"),
        ("fn main(){x:=1e}", "E0006"),
        ("fn main(){x:=1e+}", "E0006"),
        ("fn main(){x:=1.0_e2}", "E0006"),
        ("fn main(){print(1.0.sqrt(2))}", "E0207"),
        ("fn main(){print(1.0.missing())}", "E0261"),
        ("fn main(){print(f32.from_bits(i64(1)))}", "E0201"),
    ] {
        let error = dunder::check(source)
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.code, code, "{source}: {}", error.message);
    }
    for source in [
        "fn main(){x:f32=1.25;y:=x+0.5;z:f32=y}",
        "fn main(){x:f32=1.25;y:=(1.0+2.0)+x;z:f32=y}",
        "fn main(){x:=1+0.5;y:f64=x}",
        "fn main(){for i in 1..3{};x:=1_000.25e-2}",
        "fn main(){x:=f64.from_bits(1).sqrt();y:=x.to_bits()}",
        "fn root()->f32{2.25.sqrt()} fn main(){x:f32=2.25.sqrt();y:=root()}",
        "fn main(){x:f32=1.25;y:=0.25.sqrt()+x;z:f32=y;w:=1+0.25.sqrt();wide:f64=w}",
    ] {
        dunder::check(source).unwrap_or_else(|e| panic!("{}", e.render("float.ddr", source)));
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
fn arithmetic_ieee_comparisons_math_and_casts_execute_natively() {
    let out = Workspace::native(
        r#"
fn square(x:f32)->f32{x*x}
fn root()->f32{2.25.sqrt()}
fn main(){
    x:f32=1.25; y:f64=2.5
    assert(root()==1.5)
    assert(0.25.sqrt()+x==1.75);assert(x+0.25.sqrt()==1.75)
    assert(square(x)==1.5625);assert(x+0.5==1.75);assert((1.0+2.0)+x==4.25)
    assert((x<1.5)==(x<=1.25));assert((1.0<2.0)==(3.0>2.0))
    assert(1+0.5==1.5);assert(y/2==1.25);assert(-y==-2.5)
    assert(9.0.sqrt()==3.0);assert(f32(16.0).sqrt()==4.0)
    assert((-1.2).floor()==-2.0);assert((-1.2).ceil()==-1.0);assert((-1.8).trunc()==-1.0)
    assert(2.5.round()==3.0);assert((-2.5).round()==-3.0);assert(2.5.round_ties_even()==2.0)
    assert((-0.0).is_sign_negative());assert(!(-0.0).abs().is_sign_negative());assert(0.0==-0.0)
    inf:=1.0/0.0;nan:=0.0/0.0
    assert(inf.is_infinite());assert(!inf.is_finite());assert(!inf.is_nan())
    assert(nan.is_nan());assert(!nan.is_finite());assert(!nan.is_infinite());assert(nan!=nan)
    assert(!(nan==nan));assert(!(nan<0.0));assert(!(nan<=0.0));assert(!(nan>0.0));assert(!(nan>=0.0))
    assert(!(0.0<nan));assert(!(0.0<=nan));assert(!(0.0>nan));assert(!(0.0>=nan))
    assert((-1.0).sqrt().is_nan());assert(1.0.is_finite())
    assert(i32(3.9)==3);assert(i32(-3.9)==-3);assert(i32(-2147483648.9)==-2147483648)
    assert(i64(-9223372036854775808.0)==i64(-9223372036854775808))
    assert(f32(i32(16777217))==16777216.0);assert(f64(i64(9007199254740993))==9007199254740992.0)
    assert(f64(f32(0.5))==0.5);assert(f32(y)==2.5)
    assert(f64.from_bits(1)>0.0);assert(f64.from_bits(1)/2.0==0.0)
    assert(f32.from_bits(1)>0.0);assert(f32.from_bits(1)*2.0==f32.from_bits(2))
    assert(f32.from_bits(-2147483648).to_bits()==-2147483648)
    print(42)
}
"#,
    );
    assert!(
        out.status.success(),
        "status {:?}, output {:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(out.stdout, b"42\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn shortest_decimal_formatting_handles_common_values_and_ieee_extremes() {
    let out = Workspace::native(
        r#"
fn main(){
    print(0.1);print(f32(0.1));print(1.25);print(-0.0);print(1.0)
    print(1e20);print(1e-5);print(1.0/0.0);print(-1.0/0.0);print(0.0/0.0)
    print(f64.from_bits(1));print(f32.from_bits(1));print(1.7976931348623157e308)
    print(3.4028235e38);print(f32(3.4028235e38));print("value="+0.1.str())
}
"#,
    );
    assert!(
        out.status.success(),
        "status {:?}, output {:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(),"0.1\n0.1\n1.25\n-0.0\n1.0\n1e+20\n1e-5\ninf\n-inf\nnan\n5e-324\n1e-45\n1.7976931348623157e+308\n3.4028235e+38\n3.4028235e+38\nvalue=0.1\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn float_values_preserve_layout_callbacks_and_independent_register_banks() {
    let out = Workspace::native(
        r#"
struct Sample{flag:bool,small:f32,wide:f64,id:i32}
fn twice(x:f64)->f64{x*2}
fn apply(f:fn(f64)->f64,x:f64)->f64{f(x)}
fn chosen()->fn(f64)->f64{twice}
fn mixed(a:f32,b:i32,c:f64,d:f32,e:i64,f:f64,g:bool,h:f32,i:f64,j:f32,k:f64,l:f32,m:f64,n:f32,o:f64,p:f32,q:f64,r:f32,s:f64,t:i32,u:i32,v:i32,w:i32,x:i32,y:i32,z:i32,last:Sample)->f64{
    assert(g);assert(b==1&&e==2&&t==3&&u==4&&v==5&&w==6&&x==7&&y==8&&z==9)
    assert(last.flag&&last.id==42);assert(last.small==0.25&&last.wide==0.5)
    f64(a+d+h+j+l+n+p+r)+c+f+i+k+m+o+q+s
}
fn recurse(n:i32,x:f64)->f64{if n==0{x}else{recurse(n-1,x+0.25)}}
fn main(){
    original:=Sample{flag:true,small:0.25,wide:0.5,id:42}
    var changed:=original;changed.small=3.5;changed.wide=7.25
    assert(original.small==0.25);assert(original.wide==0.5)
    assert(changed.small==3.5&&changed.wide==7.25)
    var values:[f32;4]=[0.25,0.5,0.75,1.0];copy:=values;values[0]=99.0
    var total:f32=0;for value in copy.iter(){total=total+value};assert(total==2.5)
    wide:[f64;3]=[1.0,2.0,3.0];var sum:f64=0;for value in wide{sum=sum+value};assert(sum==6.0)
    assert(apply(chosen(),1.25)==2.5);assert(recurse(100,1.0)==26.0)
    assert(mixed(1.0,1,2.0,3.0,2,4.0,true,5.0,6.0,7.0,8.0,9.0,10.0,11.0,12.0,13.0,14.0,15.0,16.0,3,4,5,6,7,8,9,original)==136.0)
    exact:f32=1.0000000596046447753906250000000001
    assert(exact.to_bits()==1065353217)
    large:f64=1e300;assert(f32(large).is_infinite())
    print(42)
}
"#,
    );
    assert!(out.status.success(), "status {:?}", out.status);
    assert_eq!(out.stdout, b"42\n");
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn float_to_integer_rejects_nan_infinity_and_truncated_out_of_range_values() {
    for expr in [
        "i32(2147483648.0)",
        "i32(-2147483649.0)",
        "i64(9223372036854775808.0)",
        "i64(-9223372036854777856.0)",
        "i64(0.0/0.0)",
        "i32(1.0/0.0)",
        "i32(-1.0/0.0)",
        "i32(f32(2147483648.0))",
    ] {
        let out = Workspace::native(&format!("fn main(){{x:={expr}}}"));
        assert_eq!(out.status.code(), Some(107), "{expr}");
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
fn native_floating_results_match_independent_rust_ieee_reference() {
    let doubles = [
        0.0f64,
        -0.0,
        0.1,
        -2.75,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        f64::INFINITY,
        f64::NAN,
    ];
    let singles = [
        0.0f32,
        -0.0,
        0.1,
        -2.75,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::INFINITY,
        f32::NAN,
    ];
    let mut source = String::from("fn main(){\n");
    let mut expected = String::new();
    for &a in &doubles {
        for &b in &doubles {
            for (op, result) in [("+", a + b), ("-", a - b), ("*", a * b), ("/", a / b)] {
                let expr = format!(
                    "(f64.from_bits({}){op}f64.from_bits({}))",
                    a.to_bits() as i64,
                    b.to_bits() as i64
                );
                if result.is_nan() {
                    source.push_str(&format!("print({expr}.is_nan())\n"));
                    expected.push_str("true\n");
                } else {
                    source.push_str(&format!("print({expr}.to_bits())\n"));
                    expected.push_str(&format!("{}\n", result.to_bits() as i64));
                }
            }
        }
    }
    for &a in &singles {
        for &b in &singles {
            for (op, result) in [("+", a + b), ("-", a - b), ("*", a * b), ("/", a / b)] {
                let expr = format!(
                    "(f32.from_bits({}){op}f32.from_bits({}))",
                    a.to_bits() as i32,
                    b.to_bits() as i32
                );
                if result.is_nan() {
                    source.push_str(&format!("print({expr}.is_nan())\n"));
                    expected.push_str("true\n");
                } else {
                    source.push_str(&format!("print({expr}.to_bits())\n"));
                    expected.push_str(&format!("{}\n", result.to_bits() as i32));
                }
            }
        }
    }
    source.push('}');
    let out = Workspace::native(&source);
    assert!(out.status.success(), "status {:?}", out.status);
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn formatted_floats_round_trip_against_independent_rust_parser() {
    let mut doubles = vec![
        0.0f64,
        -0.0,
        0.1,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        1e23,
        1e-23,
    ];
    let mut singles = vec![
        0.0f32,
        -0.0,
        0.1,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        1e23,
        1e-23,
    ];
    let mut state = 0x4d595df4d0f33173u64;
    for _ in 0..192 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let d = f64::from_bits(state);
        if d.is_finite() {
            doubles.push(d);
        }
        let s = f32::from_bits((state >> 16) as u32);
        if s.is_finite() {
            singles.push(s);
        }
    }
    let mut source = String::from("fn main(){\n");
    for d in &doubles {
        source.push_str(&format!("print(f64.from_bits({}))\n", d.to_bits() as i64));
    }
    for s in &singles {
        source.push_str(&format!("print(f32.from_bits({}))\n", s.to_bits() as i32));
    }
    source.push('}');
    let out = Workspace::native(&source);
    assert!(out.status.success(), "status {:?}", out.status);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let mut lines = stdout.lines();
    for d in doubles {
        let text = lines.next().unwrap();
        assert_eq!(
            text.parse::<f64>().unwrap().to_bits(),
            d.to_bits(),
            "{d:?} -> {text}"
        );
    }
    for s in singles {
        let text = lines.next().unwrap();
        assert_eq!(
            text.parse::<f32>().unwrap().to_bits(),
            s.to_bits(),
            "{s:?} -> {text}"
        );
    }
    assert!(lines.next().is_none());
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn imported_float_types_tagged_cases_and_runtime_errors_compose() {
    let work = Workspace::new();
    fs::write(work.0.join("math.ddr"),"pub struct Point{x:f32,y:f32}\n@test{(Point{x:3.0,y:4.0})->5.0}\npub fn length(p:Point)->f32{(p.x*p.x+p.y*p.y).sqrt()}").unwrap();
    fs::write(
        work.0.join("main.ddr"),
        r#"
import math
@test { (2.0) -> 4.0; (0.5) -> 0.25 }
fn square(x:f64)->f64{x*x}
@test fn geometry(){math.length(math.Point{x:3.0,y:4.0})==5.0}
@test fn bad_cast(){x:=i32(1.0/0.0)}
@test fn bad_float_result(){square(3.0)==8.0}
@test fn after_failures(){0.1.str()=="0.1"}
"#,
    )
    .unwrap();
    let out = work.cli(&["test", "main.ddr"]);
    assert_eq!(out.status.code(), Some(1));
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        report.contains("4 passed, 2 failed"),
        "{report} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(report.contains("floating conversion out of range"));
    assert!(report.contains("bad_float_result (assertion failed)"));
}
