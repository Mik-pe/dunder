#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]

use dunder::target::Target;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-tabell-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn build(&self, target: Target) -> PathBuf {
        let binary = self.0.join(target.name());
        let result = Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(["build", "--target", target.name(), "-o"])
            .arg(&binary)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("projects/tabell"))
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap();
        success(&result);
        binary
    }

    fn run(&self, binary: &Path, args: &[&str]) -> Output {
        Command::new(binary)
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .output()
            .unwrap()
    }

    fn report(&self, binary: &Path, input: &[u8], group: &str, amount: &str) -> Output {
        let name = "rapport å 🌍.csv";
        fs::write(self.0.join(name), input).unwrap();
        self.run(binary, &[name, group, amount])
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(result: &Output) {
    assert!(
        result.status.success(),
        "status {:?}\nstdout: {}\nstderr: {}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[test]
fn standalone_project_manifest_builds_for_every_native_target_without_external_tools() {
    let work = Workspace::new();
    for target in Target::NATIVE {
        let binary = work.build(target);
        let image = fs::read(binary).unwrap();
        match target {
            Target::Wasm32 => unreachable!("tabell uses native filesystem services"),
            Target::Arm64Macos => {
                assert_eq!(&image[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
                assert_eq!(
                    u32::from_le_bytes(image[4..8].try_into().unwrap()),
                    0x0100_000c
                );
            }
            Target::Arm64Linux | Target::X86_64Linux => {
                assert_eq!(&image[..4], b"\x7fELF");
                assert_eq!(
                    u16::from_le_bytes(image[18..20].try_into().unwrap()),
                    if target == Target::Arm64Linux {
                        183
                    } else {
                        62
                    }
                );
            }
        }
    }
    let tagged = Command::new(env!("CARGO_BIN_EXE_dunder"))
        .args(["test", "-o"])
        .arg(work.0.join("tagged-tests"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("projects/tabell"))
        .env("PATH", "/no-external-tools")
        .output()
        .unwrap();
    success(&tagged);
    let report = String::from_utf8(tagged.stdout).unwrap();
    assert!(report.contains("PASS csv.ddr::csv_quotes_newlines_empty_fields_and_eof"));
    assert!(report.contains("PASS report.ddr::reports_detect_overflow_before_publishing"));
}

#[test]
fn csv_report_preserves_unicode_quotes_and_multiline_fields() {
    let work = Workspace::new();
    let binary = work.build(Target::host().unwrap());
    let input = concat!(
        "\u{feff}grupp,belopp,note\r\n",
        "\"Åsa, A\",+7,\"a \"\"quote\"\"\"\r\n",
        "\"Line\r\nbreak\",-2,note\r\n",
        "\"Åsa, A\",5,emoji 🌍\r\n",
        "\"He said \"\"hej\"\"\",4,\r\n",
        ",3,\r\n",
    );
    let result = work.report(&binary, input.as_bytes(), "grupp", "belopp");
    success(&result);
    assert!(result.stderr.is_empty());
    assert_eq!(
        result.stdout,
        concat!(
            "group,total\n",
            ",3\n",
            "\"He said \"\"hej\"\"\",4\n",
            "\"Line\r\nbreak\",-2\n",
            "\"Åsa, A\",12\n",
        )
        .as_bytes()
    );
    for header in [
        b"group,amount".as_slice(),
        b"group,amount\n",
        b"group,amount\r",
    ] {
        let result = work.report(&binary, header, "group", "amount");
        success(&result);
        assert_eq!(result.stdout, b"group,total\n");
    }
    let result = work.report(&binary, b"group,amount\ra,1\rb,2\ra,-3", "group", "amount");
    success(&result);
    assert_eq!(result.stdout, b"group,total\na,-2\nb,2\n");
}

#[test]
fn generated_reports_match_independent_rust_grouping_and_i64_extremes() {
    let work = Workspace::new();
    let binary = work.build(Target::host().unwrap());
    let groups = [
        "",
        "alpha",
        "Åsa",
        "comma,value",
        "quote\"value",
        "line\nvalue",
        "emoji 🌍",
        "Örn",
    ];
    let mut csv = String::from("memo,amount,group\r\n");
    let mut sums = BTreeMap::<&str, i64>::new();
    for row in 0..512_i64 {
        let group = groups[((row * 17 + row / 7) as usize) % groups.len()];
        let amount = (row % 13 - 6) * 2_000_000_003;
        *sums.entry(group).or_default() += amount;
        csv.push_str(&format!(
            "\"ignored, {}\",{},{}\r\n",
            row,
            amount,
            csv_field(group)
        ));
    }
    let mut expected = String::from("group,total\n");
    for (group, amount) in sums {
        expected.push_str(&format!("{},{}\n", csv_field(group), amount));
    }
    let result = work.report(&binary, csv.as_bytes(), "group", "amount");
    success(&result);
    assert_eq!(result.stdout, expected.as_bytes());
    assert!(result.stderr.is_empty());

    let result = work.report(
        &binary,
        b"group,amount\nmax,9223372036854775807\nmin,-9223372036854775808\nzero,+0\nzero,-0\ncancel,9223372036854775807\ncancel,-9223372036854775808",
        "group",
        "amount",
    );
    success(&result);
    assert_eq!(
        result.stdout,
        b"group,total\ncancel,-1\nmax,9223372036854775807\nmin,-9223372036854775808\nzero,0\n"
    );
}

#[test]
fn bad_csv_and_arithmetic_fail_without_publishing_partial_reports() {
    let work = Workspace::new();
    let binary = work.build(Target::host().unwrap());
    for (name, input) in [
        ("empty file", ""),
        ("BOM only", "\u{feff}"),
        ("duplicate header", "group,amount,group\na,1,b\n"),
        ("empty header", "group,amount,\na,1,b\n"),
        ("missing column", "group,value\na,1\n"),
        ("short row", "group,amount\na\n"),
        ("long row", "group,amount\na,1,extra\n"),
        ("blank row", "group,amount\n\n"),
        ("unterminated quote", "group,amount\n\"a,1"),
        ("quote in bare field", "group,amount\na\"b,1\n"),
        ("characters after quote", "group,amount\n\"a\"b,1\n"),
        ("empty amount", "group,amount\na,\n"),
        ("fractional amount", "group,amount\na,1.5\n"),
        ("whitespace amount", "group,amount\na, 1\n"),
        ("trailing whitespace", "group,amount\na,1 \n"),
        ("numeric separator", "group,amount\na,1_000\n"),
        ("sign only", "group,amount\na,+\n"),
        (
            "out of range positive",
            "group,amount\na,9223372036854775808\n",
        ),
        (
            "out of range negative",
            "group,amount\na,-9223372036854775809\n",
        ),
        (
            "positive sum overflow",
            "group,amount\na,9223372036854775807\na,1\n",
        ),
        (
            "negative sum overflow",
            "group,amount\na,-9223372036854775808\na,-1\n",
        ),
        ("late failure", "group,amount\na,1\nb,2\nc,invalid\n"),
    ] {
        let result = work.report(&binary, input.as_bytes(), "group", "amount");
        assert_eq!(result.status.code(), Some(1), "{name}: {result:?}");
        assert!(
            result.stdout.is_empty(),
            "{name}: partial report {result:?}"
        );
        assert!(
            String::from_utf8_lossy(&result.stderr).starts_with("tabell: line "),
            "{name}: {result:?}"
        );
    }
    for (input, line) in [
        ("group,amount\r\n\"first\r\nsecond\",3\r\nbad,x\r\n", 4),
        ("group,amount\n\"first\nsecond\"x,1", 3),
    ] {
        let result = work.report(&binary, input.as_bytes(), "group", "amount");
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).starts_with(&format!("tabell: line {line}:")),
            "physical line after multiline CSV: {result:?}"
        );
    }
}

#[test]
fn command_line_and_input_io_failures_have_useful_exit_statuses() {
    let work = Workspace::new();
    let binary = work.build(Target::host().unwrap());
    let help = work.run(&binary, &["--help"]);
    success(&help);
    assert!(!help.stdout.is_empty());
    for args in [vec![], vec!["input.csv"], vec!["a", "b", "c", "d"]] {
        let result = work.run(&binary, &args);
        assert_eq!(result.status.code(), Some(2), "{result:?}");
        assert!(!result.stderr.is_empty());
    }
    for name in ["does-not-exist.csv", "."] {
        let result = work.run(&binary, &[name, "group", "amount"]);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).starts_with("tabell:"));
    }
    let invalid = work.report(&binary, b"group,amount\n\xff,1\n", "group", "amount");
    assert_eq!(invalid.status.code(), Some(1), "{invalid:?}");
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).starts_with("tabell:"));
}
