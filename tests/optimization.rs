#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]

use dunder::target::Target;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Program(PathBuf);
impl Program {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-optimization-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn check(&self, source: &str, expected_status: i32, expected_stdout: &str) {
        let host = Target::host().unwrap();
        let mut targets = vec![(host, None)];
        if cfg!(target_os = "linux") && host != Target::Arm64Linux {
            let explicit = std::env::var_os("DUNDER_QEMU_AARCH64").map(PathBuf::from);
            if let Some(path) = &explicit {
                assert!(
                    path.is_file(),
                    "DUNDER_QEMU_AARCH64 must name an executable"
                );
            }
            if let Some(emulator) = explicit
                .into_iter()
                .chain([
                    PathBuf::from("/usr/bin/qemu-aarch64"),
                    PathBuf::from("/usr/bin/qemu-aarch64-static"),
                    PathBuf::from("/workspace/.tools/qemu/usr/bin/qemu-aarch64"),
                ])
                .find(|path| path.is_file())
            {
                targets.push((Target::Arm64Linux, Some(emulator)));
            }
        }
        for (target, emulator) in targets {
            let binary = self.0.join("program");
            let image = dunder::compile_for(source, target)
                .unwrap_or_else(|error| panic!("{}", error.render("optimization.ddr", source)));
            fs::write(&binary, image).unwrap();
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
            let stdout = self.0.join("stdout");
            let stderr = self.0.join("stderr");
            let mut command = if let Some(emulator) = emulator {
                let mut command = Command::new(emulator);
                command.arg(&binary);
                command
            } else {
                Command::new(&binary)
            };
            let mut child = command
                .env_clear()
                .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
                .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
                .spawn()
                .unwrap();
            let started = Instant::now();
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if started.elapsed() > Duration::from_secs(15) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("optimized program did not terminate on {target:?}");
                }
                thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(
                status.code(),
                Some(expected_status),
                "{target:?}: {}",
                fs::read_to_string(stderr).unwrap()
            );
            assert_eq!(
                fs::read_to_string(stdout).unwrap(),
                expected_stdout,
                "{target:?}"
            );
        }
    }
}
impl Drop for Program {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn comparison_branches_and_values_agree_at_signed_boundaries() {
    let mut source = String::new();
    for width in [32, 64] {
        source.push_str(&format!(
            "fn check{width}(a:i{width},b:i{width},expected:i32){{var mask:=0\n"
        ));
        for (bit, op) in ["==", "!=", "<", "<=", ">", ">="].iter().enumerate() {
            source.push_str(&format!(
                "value{bit}:=a{op}b\nif a{op}b{{assert(value{bit});mask=mask+{}}}else{{assert(!value{bit})}}\n",
                1 << bit
            ));
        }
        source.push_str("assert(mask==expected)\n}\n");
        source.push_str(&format!(
            "fn literal{width}(a:i{width},expected:i32){{var mask:=0\n"
        ));
        for (bit, (op, value)) in [
            ("==", 0),
            ("!=", -1),
            ("<", 4095),
            ("<=", 4096),
            (">", -4096),
            (">=", 1),
        ]
        .iter()
        .enumerate()
        {
            source.push_str(&format!("if a{op}{value}{{mask=mask+{}}}\n", 1 << bit));
        }
        source.push_str("assert(mask==expected)\n}\n");
    }
    source.push_str("fn main(){\n");
    for width in [32, 64] {
        let (min, max) = if width == 32 {
            (i64::from(i32::MIN), i64::from(i32::MAX))
        } else {
            (i64::MIN, i64::MAX)
        };
        let values = [min, min + 1, -4096, -1, 0, 1, 4095, 4096, max - 1, max];
        for a in values {
            let mask = [a == 0, a != -1, a < 4095, a <= 4096, a > -4096, a >= 1]
                .iter()
                .enumerate()
                .fold(0, |mask, (bit, value)| mask | (i32::from(*value) << bit));
            source.push_str(&format!("literal{width}({a},{mask})\n"));
            for b in values {
                let mask = [a == b, a != b, a < b, a <= b, a > b, a >= b]
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (bit, value)| mask | (i32::from(*value) << bit));
                source.push_str(&format!("check{width}({a},{b},{mask})\n"));
            }
        }
    }
    source.push_str("print(42)}");
    Program::new().check(&source, 0, "42\n");
}

#[test]
fn constant_operands_and_nested_expressions_match_independent_integer_reference() {
    let mut source = String::from(
        "fn calculate(a:i64,b:i64,c:i64)->i64{((a+4095)*(b-4096)+(a-c)*17)/7+((a*31+b)%97)}\nfn main(){\n",
    );
    let mut state = 0x123456789abcdef0u64;
    for _ in 0..256 {
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 32) % 20001) as i64 - 10000
        };
        let (a, b, c) = (next(), next(), next());
        let expected = ((a + 4095) * (b - 4096) + (a - c) * 17) / 7 + ((a * 31 + b) % 97);
        source.push_str(&format!("assert(calculate({a},{b},{c})=={expected})\n"));
    }
    source.push_str("print(42)}");
    Program::new().check(&source, 0, "42\n");
}

#[test]
fn branching_and_arithmetic_preserve_side_effect_order_and_alias_snapshots() {
    Program::new().check(
        r#"
fn mark(state:&mut i32,digit:i32,value:i32)->i32{state=state*10+digit;value}
fn truth(state:&mut i32,digit:i32,value:bool)->bool{state=state*10+digit;value}
fn change(text:&mut string)->string{text="changed"+"!";"tail"+"!"}
fn main(){
 var state:=0
 assert(0*mark(&mut state,1,77)==0)
 assert(mark(&mut state,2,3)+mark(&mut state,3,4)*mark(&mut state,4,5)==23)
 assert(state==1234)
 state=0
 if mark(&mut state,1,3)<mark(&mut state,2,4) && truth(&mut state,3,true){state=state*10+4}
 assert(state==1234)
 state=0
 if !(truth(&mut state,1,false)&&truth(&mut state,2,true)) || truth(&mut state,3,false){state=state*10+4}
 assert(state==14)
 state=0
 if (truth(&mut state,1,true)||truth(&mut state,2,false)) && truth(&mut state,3,false){assert(false)}
 assert(state==13)
 state=5
 assert(state+mark(&mut state,1,8)==13)
 assert(state==51)
 state=5
 if mark(&mut state,1,8)<state{assert(state==51)}else{assert(false)}
 state=0
 var count:=0
 while mark(&mut count,0,0)==0 && count<10{count=count+1;break}
 assert(count==1)
 var text:="old"+"!"
 assert(text+change(&mut text)=="old!tail!")
 assert(text=="changed!")
 print(42)
}
"#,
        0,
        "42\n",
    );
}

#[test]
fn optimized_literal_arithmetic_keeps_traps_and_observable_prefixes() {
    for (expression, status) in [
        ("2147483647+1", 101),
        ("-2147483648-1", 101),
        ("i64(9223372036854775807)+1", 101),
        ("i64(-9223372036854775808)-1", 101),
        ("50000*50000", 101),
        ("-2147483648/-1", 101),
        ("-2147483648%-1", 101),
        ("i64(-9223372036854775808)/-1", 101),
        ("i64(-9223372036854775808)%-1", 101),
        ("1/0", 102),
        ("7%0", 102),
        ("0*(1/0)", 102),
        ("0*fail()", 104),
    ] {
        let source =
            format!("fn fail()->i32{{assert(false);7}} fn main(){{print(17);print({expression})}}");
        Program::new().check(&source, status, "17\n");
    }
    Program::new().check(
        "fn fail()->bool{assert(false);true} fn main(){if false&&fail(){assert(false)};assert(!(false&&(1/0==0)));assert(true||(1/0==0));if true||fail(){print(42)}}",
        0,
        "42\n",
    );
}

#[test]
fn floating_conditions_keep_unordered_comparisons_and_signed_zero() {
    Program::new().check(
        r#"
fn check(a:f64,b:f64,expected:i32){
 var mask:=0
 if a==b{mask=mask+1};if a!=b{mask=mask+2}
 if a<b{mask=mask+4};if a<=b{mask=mask+8}
 if a>b{mask=mask+16};if a>=b{mask=mask+32}
 assert(mask==expected)
}
fn main(){
 nan:=0.0/0.0
 check(nan,0.0,2);check(0.0,nan,2);check(nan,nan,2)
 check(-0.0,0.0,41);check(0.0,-0.0,41)
 check(-1.0,0.0,14);check(1.0,0.0,50)
 check(1.0/0.0,1.0/0.0,41)
 assert((-0.0*1.0).is_sign_negative())
 assert((0.0*(-1.0)).is_sign_negative())
 if !(nan<0.0) && !(nan>=0.0){print(42)}else{assert(false)}
}
"#,
        0,
        "42\n",
    );
}

#[test]
fn inline_parameters_are_eager_and_loop_homes_survive_calls_and_borrows() {
    Program::new().check(
        r#"
struct Point { x:f32, y:f32 }
fn point(x:f32,y:f32)->Point{Point{x:x,y:y}}
fn bump(state:&mut i32)->i32{state=state+1;state}
fn pair(left:i32,right:i32)->i32{left*10+right}
fn twice(a:i32)->i32{a+a}
fn nested(n:i32)->i32{
    if n==0{return 0}
    var acc:=0
    var i:=0
    while i<n{acc=acc+i;i=i+1}
    return acc+nested(n-1)
}
fn main(){
    var p:=point(3.0,8.0)
    p=point(p.y,p.x)
    assert(p.x==8.0 && p.y==3.0)
    var count:=1
    assert(pair(count,bump(&mut count))==12)
    assert(pair(bump(&mut count),twice(bump(&mut count)))==38)
    assert(count==4)
    var sum:=0
    while count<10 {
        sum=sum+nested(4)
        next:=bump(&mut count)
    }
    assert(count==10 && sum==60)
    print(42)
}
"#,
        0,
        "42\n",
    );
    Program::new().check(
        "fn first(a:i32,b:i32)->i32{a} fn main(){print(17);print(first(42,1/0))}",
        102,
        "17\n",
    );
}

#[test]
fn transferred_buffers_and_strings_keep_independent_owners() {
    Program::new().check(
        r#"
fn words(input:string)->[]string{
    var data:[]string=[input+"1",input+"2"]
    if input=="early"{return data}
    data.push(input+"3")
    data
}
fn identity(input:string)->string{input}
fn main(){
    var i:=0
    while i<200{
        var original:=words("early")
        var copy:=original
        original[0]="new"
        assert(copy[0]=="early1" && original[0]=="new")
        var moved:=words("late")
        assert(moved.pop()=="late3")
        var transient:=identity("heap"+i.str())
        assert(transient.len>=5)
        i=i+1
    }
    print(42)
}
"#,
        0,
        "42\n",
    );
}

#[test]
fn packed_unsigned_scalars_keep_width_across_native_operations() {
    Program::new().check(
        r#"
        fn set_byte(value:&mut u8) { value = 250 }
        fn lots(a:u8,b:u8,c:u8,d:u8,e:u8,f:u8,g:u8,h:u8,i:u8,j:u16,k:u32)->u32 {
            u32(i) + u32(j) + k
        }
        fn main(){
            var bytes:[]u8 = [1,2,3]
            set_byte(&mut bytes[1])
            var clone := bytes
            clone[1] = 7
            assert(bytes[1] == 250 && bytes[2] == 3)
            var halves:[u16;3] = [65000,17,50000]
            halves[1] = 54321
            assert(halves[0] == 65000 && halves[1] == 54321 && halves[2] == 50000)
            assert(lots(0,1,2,3,4,5,6,7,8,60000,4000000000) == 4000060008)
            max:u64 = 18446744073709551615
            assert(max > 9223372036854775808 && max / 3 == 6148914691236517205)
            assert(max % 7 == 1 && max.wrapping_add(1) == 0)
            assert(u8(130).shl(1) == 4 && u16(0).bit_not() == 65535)
            high:u64 = 9223372036854775808
            assert(u64(f64(high)) == high && u64(f32(high)) == high)
            assert(u64(4611686018427387904) * 3 == 13835058055282163712)
            print(max)
            print(max.str())
        }
        "#,
        0,
        "18446744073709551615\n18446744073709551615\n",
    );
}

#[test]
fn unsigned_arithmetic_and_conversions_trap_before_wraparound() {
    let program = Program::new();
    for expr in [
        "u8(255) + 1",
        "u16(0) - 1",
        "u32(65536) * 65536",
        "u64(9223372036854775808) * 2",
        "u64(18446744073709551615) + 1",
        "u8(u16(256))",
        "u64(i32(-1))",
        "i64(u64(9223372036854775808))",
    ] {
        program.check(&format!("fn main(){{print({expr})}}"), 101, "");
    }
}
