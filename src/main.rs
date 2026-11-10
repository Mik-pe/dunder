use std::os::unix::fs::PermissionsExt;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    time::Instant,
};

fn manifest() -> Result<(PathBuf, String), String> {
    let mut dir = env::current_dir().map_err(|e| e.to_string())?;
    loop {
        let path = dir.join("dunder.toml");
        if path.is_file() {
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let mut name = None;
            let mut root = None;
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
            return Ok((dir.join(root.ok_or("manifest requires root")?), name));
        }
        if !dir.pop() {
            return Err(
                "no dunder.toml found; run 'dunder build file.dunder' or create a manifest".into(),
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

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        println!("Dunder — typed source to native ARM64\n\n  dunder build [file.dunder] [-o output] [--timings]\n  dunder check [file.dunder]\n\nWithout a file, finds the nearest dunder.toml. Target: arm64-macos.\nDeclarations: x := 42, x: i32 = 42, var x := 0. Values are immutable by default.");
        return Ok(());
    }
    let command = &args[0];
    if !matches!(command.as_str(), "build" | "check") {
        return Err(format!("unknown command '{command}'; use build or check"));
    }
    let mut source_path = None;
    let mut output = None;
    let mut timings = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("-o requires a path")?));
            }
            "--timings" => timings = true,
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
        manifest()?
    };
    let source =
        fs::read_to_string(&source_path).map_err(|e| format!("{}: {e}", source_path.display()))?;
    let read_time = start.elapsed();
    let checked = dunder::check(&source)
        .map_err(|e| e.render(&source_path.display().to_string(), &source))?;
    let check_time = start.elapsed() - read_time;
    if command == "check" {
        println!("checked {}", source_path.display());
        return Ok(());
    }
    let image = dunder::native::compile(&checked)?;
    let native_time = start.elapsed() - read_time - check_time;
    let output = output.unwrap_or_else(|| PathBuf::from("build/arm64-macos").join(name));
    if let Ok(existing) = fs::canonicalize(&output) {
        if existing == fs::canonicalize(&source_path).map_err(|e| e.to_string())? {
            return Err("output must not overwrite the source file".into());
        }
    }
    publish(&output, &image).map_err(|e| format!("{}: {e}", output.display()))?;
    println!("built {}", output.display());
    if timings {
        eprintln!(
            "read={:.3}ms parse+check={:.3}ms native+sign={:.3}ms total={:.3}ms",
            read_time.as_secs_f64() * 1000.,
            check_time.as_secs_f64() * 1000.,
            native_time.as_secs_f64() * 1000.,
            start.elapsed().as_secs_f64() * 1000.
        );
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}
