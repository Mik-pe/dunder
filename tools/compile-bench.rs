//! Host-only measurement driver. Never part of Dunder's compiler pipeline.
use std::{env, fs, path::PathBuf, process::Command, time::Instant};
struct Workspace(PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tokens(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut count = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
        } else if i + 1 < bytes.len()
            && matches!(
                &bytes[i..i + 2],
                b":=" | b"->" | b"==" | b"!=" | b"<=" | b">=" | b"&&" | b"||" | b".."
            )
        {
            i += 2;
        } else {
            i += 1;
        }
        count += 1;
    }
    count
}
fn main() {
    let args: Vec<_> = env::args().collect();
    assert!(
        args.len() >= 3,
        "compile-bench COMPILER REPORT.csv [ROUNDS]"
    );
    let compiler = fs::canonicalize(&args[1]).unwrap();
    let rounds: usize = args.get(3).map_or(30, |s| s.parse().unwrap());
    let root = env::temp_dir().join(format!("dunder-compile-bench-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _workspace = Workspace(root.clone());
    assert!(rounds > 0);
    let mut token_count = 0;
    let mut lines = 0;
    let mut bytes = 0;
    let mut main = String::new();
    for module in 0..20 {
        main += &format!("import m{module}\n");
        let mut source = String::new();
        for f in 0..50 {
            source += &format!("pub fn item{f}(x: i64) -> i64 {{\n    var value := x\n");
            for _ in 0..96 {
                source += "    value = value + 1\n";
            }
            source += "    return value\n}\n";
        }
        lines += source.lines().count();
        bytes += source.len();
        token_count += tokens(&source);
        fs::write(root.join(format!("m{module}.ddr")), source).unwrap();
    }
    main += "fn main() {\n    var total: i64 = 0\n";
    for module in 0..20 {
        for f in 0..50 {
            main += &format!(
                "    total = total + m{module}.item{f}({})\n",
                module * 50 + f
            );
        }
    }
    main += "    print(total)\n}\n";
    lines += main.lines().count();
    bytes += main.len();
    token_count += tokens(&main);
    fs::write(root.join("main.ddr"), main).unwrap();
    let mut report = format!("round,lines,source_bytes,tokens,functions,modules,elapsed_ms,image_bytes,compiler_phases\n");
    let mut times = Vec::new();
    for round in 0..rounds {
        let out = root.join(format!("out-{round}"));
        let start = Instant::now();
        let result = Command::new(&compiler)
            .args(["build", "main.ddr", "-o"])
            .arg(&out)
            .arg("--timings")
            .env("PATH", "/no-external-tools")
            .current_dir(&root)
            .output()
            .unwrap();
        let ms = start.elapsed().as_secs_f64() * 1000.;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let run = Command::new(&out).env_clear().output().unwrap();
        assert!(run.status.success());
        assert_eq!(run.stdout, b"595500\n");
        let size = fs::metadata(&out).unwrap().len();
        let phases = String::from_utf8_lossy(&result.stderr)
            .trim()
            .replace(',', ";");
        report +=
            &format!("{round},{lines},{bytes},{token_count},1001,21,{ms:.3},{size},{phases}\n");
        times.push(ms);
        fs::remove_file(out).unwrap();
    }
    times.sort_by(f64::total_cmp);
    let median = times[times.len() / 2];
    let p95 = times[(times.len() * 95).div_ceil(100) - 1];
    println!("generated stress corpus: {lines} lines, {bytes} bytes, {token_count} tokens, 1001 functions, 21 modules");
    println!("{rounds} fresh processes, no compiler cache, warm OS cache, one worker");
    println!(
        "median={median:.3}ms p95={p95:.3}ms p95_lines_per_second={:.0}",
        lines as f64 / (p95 / 1000.)
    );
    fs::write(PathBuf::from(&args[2]), report).unwrap();
}
