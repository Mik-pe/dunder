//! Host-side comparison driver. It is not part of the compiler or runtime.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

struct Case {
    name: &'static str,
    count: usize,
    argument: String,
    expected: i64,
}
fn cases() -> Vec<Case> {
    let seed = 1729i64;
    let numeric_count = 1_000_000;
    let mut state = seed;
    let mut numeric = 0;
    for _ in 0..numeric_count {
        state = (state * 1664525 + 1013904223) % 2147483647;
        numeric += state;
    }
    let scan_count = 3000;
    let mut values: Vec<_> = (0..1024).map(|i| (i * 17 + seed) % 1009).collect();
    let mut scan = 0;
    for i in 0..scan_count {
        values[i % 1024] = i as i64;
        scan += values.iter().sum::<i64>();
    }
    let phrase = "Dunder läser UTF-8: åäö, Ελληνικά och 🌍.\nSäkra värden överlever sina anrop; loopar räknar byte och runor.\tNative kod utan externa byggverktyg. ";
    let text = phrase.repeat(4);
    let text_count = 2000;
    let text_sum = (text
        .split([' ', '\n', '\t'])
        .filter(|s| !s.is_empty())
        .count() as i64
        * 1_000_000
        + text.chars().map(|c| c as i64).sum::<i64>())
        * text_count as i64;
    let sort_count = 120;
    let mut sort = 0;
    for repeat in 0..sort_count {
        let mut values: Vec<_> = (0..256)
            .map(|i| ((255 - i) * 17 + seed + repeat as i64) % 1009)
            .collect();
        values.sort_unstable();
        sort += values
            .iter()
            .enumerate()
            .map(|(i, v)| *v * (i as i64 + 1))
            .sum::<i64>();
    }
    vec![
        Case {
            name: "numeric",
            count: numeric_count,
            argument: seed.to_string(),
            expected: numeric,
        },
        Case {
            name: "scan",
            count: scan_count,
            argument: seed.to_string(),
            expected: scan,
        },
        Case {
            name: "text",
            count: text_count,
            argument: text,
            expected: text_sum,
        },
        Case {
            name: "sort",
            count: sort_count,
            argument: seed.to_string(),
            expected: sort,
        },
    ]
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037, |v, b| {
        (v ^ u64::from(*b)).wrapping_mul(1099511628211)
    })
}
fn measure(program: &Path, case: &Case) -> f64 {
    let start = Instant::now();
    let output = Command::new(program)
        .arg(case.count.to_string())
        .arg(&case.argument)
        .env_clear()
        .env("PATH", "/no-external-tools")
        .output()
        .unwrap();
    let elapsed = start.elapsed().as_secs_f64() * 1000.;
    assert!(
        output.status.success(),
        "{}: {:?}: {}",
        case.name,
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        format!("{}\n", case.expected).as_bytes(),
        "{} result mismatch",
        case.name
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    elapsed
}
fn quantile(samples: &[f64], percentile: f64) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    if percentile == 0.5 && sorted.len().is_multiple_of(2) {
        return (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.;
    }
    sorted[(percentile * sorted.len() as f64).ceil() as usize - 1]
}
fn main() {
    let mut args: Vec<_> = env::args().collect();
    let c_mode = args.get(1).map(String::as_str);
    let trap_overflow = c_mode == Some("--compare-c");
    let c_reference = trap_overflow || c_mode == Some("--compare-c-unchecked");
    let mut c_flags = vec!["-std=c11", "-O3"];
    if trap_overflow {
        c_flags.push("-ftrapv");
    }
    if c_reference {
        args.remove(1);
    }
    assert!(
        (4..=5).contains(&args.len()),
        "runtime-bench [--compare-c|--compare-c-unchecked] BASELINE_COMPILER CANDIDATE REPORT_DIR [ROUNDS]"
    );
    let compilers = [
        fs::canonicalize(&args[1]).unwrap(),
        fs::canonicalize(&args[2]).unwrap(),
    ];
    let report = PathBuf::from(&args[3]);
    fs::create_dir_all(&report).unwrap();
    let report = fs::canonicalize(report).unwrap();
    let rounds: usize = args.get(4).map_or(30, |s| s.parse().unwrap());
    assert!(rounds > 0);
    let root = env::current_dir().unwrap();
    let work = root.join("build/runtime-bench");
    fs::create_dir_all(&work).unwrap();
    let labels = if c_reference {
        ["c", "dunder"]
    } else {
        ["baseline", "candidate"]
    };
    let mut metadata = format!("rounds={rounds}\ntarget=host\nprofile=default\norder=round-even baseline,candidate; round-odd candidate,baseline\ncache=no Dunder cache; warm OS file cache\nclock=wall clock including process startup, args and result output\nchecksums=FNV-1a 64-bit (reproducibility, not cryptographic)\n");
    if c_reference {
        let version = Command::new(&compilers[0])
            .arg("--version")
            .output()
            .unwrap();
        metadata += &format!(
            "baseline=C\nc_flags={}\nc_version={}\nc_source_fnv={:016x}\n",
            c_flags.join(" "),
            String::from_utf8_lossy(&version.stdout).trim(),
            hash(&fs::read(root.join("benchmarks/runtime/reference.c")).unwrap())
        );
    }
    for (i, compiler) in compilers.iter().enumerate() {
        metadata += &format!(
            "{}_compiler={}\n{}_compiler_fnv={:016x}\n",
            labels[i],
            compiler.display(),
            labels[i],
            hash(&fs::read(compiler).unwrap())
        );
    }
    for (key, command, arguments) in [
        ("host", "uname", vec!["-a"]),
        ("candidate_checkout", "git", vec!["rev-parse", "HEAD"]),
        ("working_tree", "git", vec!["status", "--short"]),
    ] {
        if let Ok(output) = Command::new(command).args(arguments).output() {
            metadata += &format!("{key}={}\n", String::from_utf8_lossy(&output.stdout).trim());
        }
    }
    if let Ok(cpu) = fs::read_to_string("/proc/cpuinfo") {
        if let Some(model) = cpu.lines().find(|line| line.starts_with("model name")) {
            metadata += &format!("cpu={model}\n");
        }
    }
    let mut csv = String::from("case,round,position,variant,elapsed_ms,image_bytes,checksum\n");
    let mut summary = String::from("case,baseline_median_ms,candidate_median_ms,candidate_over_baseline,baseline_p95_ms,candidate_p95_ms,baseline_image_bytes,candidate_image_bytes\n");
    for case in cases() {
        let source = root
            .join("benchmarks/runtime")
            .join(format!("{}.ddr", case.name));
        metadata += &format!(
            "{}_count={}\n{}_argument={:?}\n{}_expected={}\n{}_source_fnv={:016x}\n",
            case.name,
            case.count,
            case.name,
            case.argument,
            case.name,
            case.expected,
            case.name,
            hash(&fs::read(&source).unwrap())
        );
        let binaries: Vec<_> = labels
            .iter()
            .map(|label| work.join(format!("{}-{label}", case.name)))
            .collect();
        let mut sizes = [0; 2];
        for i in 0..2 {
            let start = Instant::now();
            let mut compiler = Command::new(&compilers[i]);
            if c_reference && i == 0 {
                compiler
                    .args(&c_flags)
                    .arg(format!("-DCASE_{}", case.name))
                    .arg(root.join("benchmarks/runtime/reference.c"));
            } else {
                compiler
                    .arg("build")
                    .arg(&source)
                    .env_clear()
                    .env("PATH", "/no-external-tools");
            }
            let output = compiler.arg("-o").arg(&binaries[i]).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let image = fs::read(&binaries[i]).unwrap();
            sizes[i] = image.len();
            metadata += &format!(
                "{}_{}_single_build_ms={:.6}\n{}_{}_image_fnv={:016x}\n",
                case.name,
                labels[i],
                start.elapsed().as_secs_f64() * 1000.,
                case.name,
                labels[i],
                hash(&image)
            );
            measure(&binaries[i], &case); // Untimed-in-report warmup also verifies the independent reference.
        }
        let mut times = [Vec::new(), Vec::new()];
        for round in 0..rounds {
            let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
            for (position, i) in order.into_iter().enumerate() {
                let ms = measure(&binaries[i], &case);
                times[i].push(ms);
                csv += &format!(
                    "{},{round},{position},{},{ms:.6},{},{}\n",
                    case.name, labels[i], sizes[i], case.expected
                );
            }
        }
        let medians = [quantile(&times[0], 0.5), quantile(&times[1], 0.5)];
        let ratio = medians[1] / medians[0];
        summary += &format!(
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{}\n",
            case.name,
            medians[0],
            medians[1],
            ratio,
            quantile(&times[0], 0.95),
            quantile(&times[1], 0.95),
            sizes[0],
            sizes[1]
        );
        println!(
            "{}: {:.3}ms -> {:.3}ms ({ratio:.3}x), {} -> {} bytes",
            case.name, medians[0], medians[1], sizes[0], sizes[1]
        );
        fs::write(report.join("samples.csv"), &csv).unwrap();
        fs::write(report.join("summary.csv"), &summary).unwrap();
        fs::write(report.join("metadata.txt"), &metadata).unwrap();
    }
}
