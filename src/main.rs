use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
mod test_runner;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant},
};

fn manifest(testing: bool) -> Result<(PathBuf, String), String> {
    let mut dir = env::current_dir().map_err(|e| e.to_string())?;
    loop {
        let path = dir.join("dunder.toml");
        if path.is_file() {
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let mut name = None;
            let mut root = None;
            let mut test_root = None;
            for (i, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (key, value) = line.split_once('=').ok_or_else(|| {
                    format!(
                        "{}:{}: expected name = \"...\" or root = \"...\"",
                        path.display(),
                        i + 1
                    )
                })?;
                let value = value.trim();
                if !(value.starts_with('"') && value.ends_with('"') && value.len() >= 2) {
                    return Err(format!(
                        "{}:{}: manifest values must be quoted strings",
                        path.display(),
                        i + 1
                    ));
                }
                let value = value[1..value.len() - 1].to_owned();
                match key.trim() {
                    "name" if name.is_none() => name = Some(value),
                    "root" if root.is_none() => root = Some(value),
                    "test_root" if test_root.is_none() => test_root = Some(value),
                    _ => {
                        return Err(format!(
                            "{}:{}: unknown or duplicate key",
                            path.display(),
                            i + 1
                        ))
                    }
                }
            }
            let name = name.ok_or("manifest requires name")?;
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err("manifest name must contain only letters, digits, '_' or '-'".into());
            }
            let root = root.ok_or("manifest requires root")?;
            let selected = if testing {
                test_root.unwrap_or(root)
            } else {
                root
            };
            return Ok((dir.join(selected), name));
        }
        if !dir.pop() {
            return Err(
                "no dunder.toml found; run 'dunder build file.dn' or create a manifest".into(),
            );
        }
    }
}

fn publish(path: &Path, image: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let stem = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(".{stem}.{}.tmp", process::id()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| {
        f.write_all(image)?;
        f.set_permissions(fs::Permissions::from_mode(0o755))?;
        f.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        println!("Dunder — typed source to native ARM64\n\n  dunder build [file.dn] [-o output] [--timings]\n  dunder run [file.dn] [-o output] [--timings]\n  dunder check [file.dn]\n  dunder test [file.dn] [-o directory] [--timeout-ms 5000]\n\nWithout a file, finds the nearest dunder.toml. Target: arm64-macos.\nDeclarations: x := 42, x: i32 = 42, var x := 0. Values are immutable by default.");
        return Ok(0);
    }
    let command = &args[0];
    if !matches!(command.as_str(), "build" | "check" | "run" | "test") {
        return Err(format!(
            "unknown command '{command}'; use build, run, check or test"
        ));
    }
    let mut source_path = None;
    let mut output = None;
    let mut timings = false;
    let mut timeout = 5000;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("-o requires a path")?));
            }
            "--timings" => timings = true,
            "--timeout-ms" if command == "test" => {
                i += 1;
                timeout = args
                    .get(i)
                    .ok_or("--timeout-ms requires milliseconds")?
                    .parse::<u64>()
                    .map_err(|_| "invalid --timeout-ms value")?;
                if timeout == 0 {
                    return Err("--timeout-ms must be positive".into());
                }
            }
            arg if arg.starts_with('-') => return Err(format!("unknown option '{arg}'")),
            _ if source_path.is_none() => source_path = Some(PathBuf::from(&args[i])),
            _ => return Err("expected a single source file".into()),
        }
        i += 1;
    }
    let start = Instant::now();
    let (source_path, name) = if let Some(p) = source_path {
        let n = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        (p, n)
    } else {
        manifest(command == "test")?
    };
    let loaded = dunder::modules::load(&source_path).map_err(|e| e.render())?;
    let module_count = loaded.sources.files.len();
    let load_time = start.elapsed();
    let checked = if command == "test" {
        loaded.check_tests()
    } else {
        loaded.check()
    }
    .map_err(|e| e.render())?;
    let check_time = start.elapsed() - load_time;
    if command == "test" {
        let output = output.unwrap_or_else(|| PathBuf::from("build/arm64-macos/tests").join(name));
        let result = test_runner::run(&checked, &output, Duration::from_millis(timeout))?;
        if timings {
            eprintln!(
                "modules={module_count} load+parse={:.3}ms check={:.3}ms test-total={:.3}ms",
                load_time.as_secs_f64() * 1000.,
                check_time.as_secs_f64() * 1000.,
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        return Ok(result);
    }
    if command == "check" {
        println!("checked {} ({module_count} modules)", source_path.display());
        return Ok(0);
    }
    let image = checked.compile().map_err(|e| checked.render(&e))?;
    let native_time = start.elapsed() - load_time - check_time;
    let output = output.unwrap_or_else(|| PathBuf::from("build/arm64-macos").join(name));
    if let Ok(existing) = fs::canonicalize(&output) {
        if checked.sources.files.iter().any(|s| s.path == existing) {
            return Err("output must not overwrite the source file".into());
        }
    }
    publish(&output, &image).map_err(|e| format!("{}: {e}", output.display()))?;
    if command == "run" {
        eprintln!("built {}", output.display());
    } else {
        println!("built {}", output.display());
    }
    if timings {
        eprintln!(
            "modules={module_count} load+parse={:.3}ms check={:.3}ms native+sign={:.3}ms total={:.3}ms",
            load_time.as_secs_f64() * 1000.,
            check_time.as_secs_f64() * 1000.,
            native_time.as_secs_f64() * 1000.,
            start.elapsed().as_secs_f64() * 1000.
        );
    }
    if command == "run" {
        let executable = fs::canonicalize(&output).map_err(|e| e.to_string())?;
        let status = process::Command::new(executable)
            .status()
            .map_err(|e| format!("could not run {}: {e}", output.display()))?;
        return Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)));
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
