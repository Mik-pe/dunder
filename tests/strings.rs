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
            "dunder-strings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn native(source: &str) -> std::process::Output {
        use std::os::unix::fs::PermissionsExt;
        let work = Self::new();
        let image =
            dunder::compile(source).unwrap_or_else(|e| panic!("{}", e.render("test.dn", source)));
        let path = work.0.join("program");
        fs::write(&path, image).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let signed = Command::new("/usr/bin/codesign")
            .arg("--verify")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            signed.status.success(),
            "{}",
            String::from_utf8_lossy(&signed.stderr)
        );
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
fn string_and_rune_types_and_utf8_literals_are_checked() {
    for (source, code) in [
        (r#"fn main(){x:i32="hello"}"#, "E0201"),
        (r#"fn main(){x:string=42}"#, "E0201"),
        (r#"fn main(){print("hello"+42)}"#, "E0201"),
        (r#"fn main(){print("a"-"b")}"#, "E0206"),
        (r#"fn main(){x:rune=65}"#, "E0201"),
        (r#"fn main(){print(`a`+`b`)}"#, "E0206"),
        (r#"fn main(){var s:="hello";s[0]=72}"#, "E0221"),
        (r#"fn main(){s:="hello";print(s[true])}"#, "E0201"),
        (r#"fn main(){for r in "hello"{print(r)}}"#, "E0222"),
        (r#"fn main(){print("hello".runes(1))}"#, "E0207"),
        (r#"fn main(){print("hello".str(1))}"#, "E0207"),
        (r#"fn main(){print("\xFF")}"#, "E0005"),
        (r#"fn main(){print("\xC0\xAF")}"#, "E0005"),
        (r#"fn main(){print("\u{D800}")}"#, "E0005"),
        (r#"fn main(){print("\u{110000}")}"#, "E0005"),
        (r#"fn main(){print("\u{}")}"#, "E0005"),
        (r#"fn main(){print("\u{1234567}")}"#, "E0005"),
        (r#"fn main(){print("\u12")}"#, "E0005"),
        (r#"fn main(){print("\xF")}"#, "E0005"),
        (r#"fn main(){print("\q")}"#, "E0004"),
        (r#"fn main(){print(``)}"#, "E0005"),
        (r#"fn main(){print(`ab`)}"#, "E0005"),
    ] {
        let error = dunder::check(source)
            .err()
            .unwrap_or_else(|| panic!("incorrectly accepted {source}"));
        assert_eq!(error.code, code, "{source}: {}", error.message);
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_strings_support_values_returns_concatenation_slices_and_embedded_nul() {
    let out = Workspace::native(
        r#"
fn greet(name:string)->string { 'Hej, ' + name + '!' }
fn identity(s:string)->string { s }
fn selected()->range { 7..11 }
fn main(){
    original := "åäö 🌍"
    print(greet(identity(original)))
    print(original.len); print(original.len()); print(len(original))
    print(original.runes().len); print(original.runes().len())
    print(original[0..6]); print(original[7..11]); print(original[11..11])
    print(original[0]); print(original.bytes()[1]); print(original.runes()[4])
    print("a\0b"); print("\xC3\xA5\u00E4\u{F6}")
    print("\"'`\\\t\r\n")
    assert(greet("Åsa")=="Hej, Åsa!")
    assert(""+""==""); assert("å"+""=="å"); assert(""+"å"=="å")
    assert("a\0b"!="a"); assert("same"=="sa"+"me")
    assert("a"<"b"); assert("ab"<"abc"); assert("å">"z"); assert("a">="a"); assert("b"<="b")
    span:=7..11; assert(original[span]=="🌍"); assert(original[selected()]=="🌍")
}
"#,
    );
    assert!(
        out.status.success(),
        "status {:?}, stdout {:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        out.stdout,
        "Hej, åäö 🌍!\n11\n11\n11\n5\n5\nåäö\n🌍\n\n195\n165\n🌍\na\0b\nåäö\n\"'`\\\t\r\n\n"
            .as_bytes()
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn rune_and_byte_iteration_preserve_scalar_boundaries_and_loop_control() {
    let out = Workspace::native(
        r#"
fn main(){
    var text := "Aå€🌍"
    snapshot := text.runes()
    text = "changed"
    var sum:=0
    for r in snapshot { sum=sum+i32(r); if r==`€`{continue}; print(r) }
    print(sum)
    for b in "å".bytes(){print(b)}
    for r in "".runes(){assert(false)}
    for r in "abc".runes(){print(r);break}
    var count:=0
    for r in "å🌍".runes(){for b in r.str().bytes(){count=count+1}}
    print(count)
    assert(snapshot[3]==`🌍`)
    assert(`e`!=`é`)
    assert("é".runes().len==1)
    assert("e\u{301}".runes().len==2)
}
"#,
    );
    assert!(out.status.success(), "status {:?}", out.status);
    let sum = 'A' as u32 + 'å' as u32 + '€' as u32 + '🌍' as u32;
    assert_eq!(
        out.stdout,
        format!("A\nå\n🌍\n{sum}\n195\n165\na\n6\n").as_bytes()
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn strings_and_runes_work_in_structs_arrays_callbacks_and_stack_arguments() {
    let out = Workspace::native(
        r#"
struct Label{text:string,enabled:bool,marker:rune,count:i64}
fn decorate(s:string)->string { "["+s+"]" }
fn apply(f:fn(string)->string,s:string)->string { f(s) }
fn many(a:string,b:rune,c:bool,d:i32,e:i64,f:string,g:rune,h:i32,i:string,j:rune,k:bool)->string{
    assert(c&&k);assert(d+h==42);assert(e==5000000000)
    a+b.str()+f+g.str()+i+j.str()
}
fn main(){
    var s:="before";copy:=s;s="after";print(copy)
    original:=Label{text:apply(decorate,"å"),enabled:true,marker:`🌍`,count:5000000000}
    var changed:=original;changed.text="other";changed.marker=`€`
    print(original.text);print(original.marker);print(changed.text);print(changed.marker)
    var words:[string;3]=["å","ä","ö"];snapshot:=words;words[0]="x";for word in snapshot{print(word)}
    var markers:[rune;2]=[`å`,`🌍`];markers[0]=`€`;for r in markers.iter(){print(r)}
    print(many("a",`å`,true,20,5000000000,"b",`€`,22,"c",`🌍`,true))
}
"#,
    );
    assert!(out.status.success(), "status {:?}", out.status);
    assert_eq!(
        out.stdout,
        "before\n[å]\n🌍\nother\n€\nå\nä\nö\n€\n🌍\naåb€c🌍\n".as_bytes()
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn scalar_formatting_and_rune_conversion_cover_full_ranges() {
    let out = Workspace::native(
        r#"
fn main(){
    print(0.str()); print((-2147483648).str()); print(i64(-9223372036854775808).str())
    print(i64(9223372036854775807).str()); print(true.str()); print(false.str())
    for value in [0,127,128,2047,2048,55295,57344,65535,65536,1114111]{
        r:=rune(value); text:=r.str(); assert(text.runes().len==1); assert(text.runes()[0]==r); assert(i32(r)==value)
    }
    print("ok")
}
"#,
    );
    assert!(
        out.status.success(),
        "status {:?}, output {:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        out.stdout,
        b"0\n-2147483648\n-9223372036854775808\n9223372036854775807\ntrue\nfalse\nok\n"
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn invalid_string_indices_boundaries_and_unicode_scalars_fail_predictably() {
    for (expression, status) in [
        ("\"å\"[-1]", 103),
        ("\"å\"[2]", 103),
        ("\"\"[0]", 103),
        ("\"å\".runes()[-1]", 103),
        ("\"å\".runes()[1]", 103),
        ("\"å\"[-1..1]", 103),
        ("\"å\"[1..0]", 103),
        ("\"å\"[0..3]", 103),
        ("\"å\"[1..2]", 106),
        ("\"å\"[0..1]", 106),
        ("\"å\"[1..1]", 106),
        ("rune(-1)", 106),
        ("rune(55296)", 106),
        ("rune(57343)", 106),
        ("rune(1114112)", 106),
    ] {
        let out = Workspace::native(&format!("fn main(){{value:={expression}}}"));
        assert_eq!(out.status.code(), Some(status), "{expression}");
    }
    let out = Workspace::native("fn main(){var s:=\"x\";for i in 0..25{s=s+s}}");
    assert_eq!(out.status.code(), Some(105));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_unicode_matches_independent_rust_byte_and_scalar_reference() {
    let samples = [
        "",
        "ASCII",
        "räksmörgås",
        "Ελλάδα",
        "日本語",
        "العربية",
        "a\0b",
        "🌍🚀",
        "e\u{301}",
        "👩‍💻",
        "\u{10ffff}",
    ];
    let mut source = String::from("fn main(){\n");
    let mut expected = String::new();
    for sample in samples {
        let literal = sample
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\0', "\\0");
        source.push_str(&format!("{{s:=\"{literal}\";print(s.len);print(s.runes().len);for r in s.runes(){{print(i32(r))}};for b in s.bytes(){{print(b)}}\n"));
        expected.push_str(&format!("{}\n{}\n", sample.len(), sample.chars().count()));
        for r in sample.chars() {
            expected.push_str(&format!("{}\n", r as u32));
        }
        for b in sample.bytes() {
            expected.push_str(&format!("{b}\n"));
        }
        let boundaries: Vec<_> = sample
            .char_indices()
            .map(|(i, _)| i)
            .chain([sample.len()])
            .collect();
        for pair in boundaries.windows(2) {
            source.push_str(&format!(
                "print(s[{}..{}].runes()[0].str()==s.runes()[{}].str())\n",
                pair[0],
                pair[1],
                sample[..pair[0]].chars().count()
            ));
            expected.push_str("true\n");
        }
        source.push_str("}\n");
    }
    source.push('}');
    let out = Workspace::native(&source);
    assert!(out.status.success(), "status {:?}", out.status);
    assert_eq!(out.stdout, expected.as_bytes());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn string_module_tests_use_real_runtime_values_and_report_unicode_failures() {
    let work = Workspace::new();
    fs::write(
        work.0.join("text.dn"),
        r#"
pub struct Label{text:string}
@test{("åäö")->"[åäö]";("🌍")->"[🌍]"}
pub fn decorate(s:string)->string {"["+s+"]"}
"#,
    )
    .unwrap();
    fs::write(
        work.0.join("main.dn"),
        r#"
import text
@test{(text.Label{text:"värld"})->"[värld]"}
fn label(s:text.Label)->string{text.decorate(s.text)}
@test fn unicode_failure(){s:="å";print(s[1..2])}
@test fn continues(){text.decorate("x")=="[x]"}
@test {("x")->"wrong"}
fn intentionally_wrong(s:string)->string{s+"!"}
"#,
    )
    .unwrap();
    let out = work.cli(&["test", "main.dn"]);
    assert_eq!(out.status.code(), Some(1));
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        report.contains("3 passed, 2 failed"),
        "{report} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(report.contains("invalid Unicode boundary or scalar"));
    assert!(report.contains("intentionally_wrong (assertion failed)"));
}
