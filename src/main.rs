use std::os::unix::{
    fs::{MetadataExt, PermissionsExt},
    process::ExitStatusExt,
};
mod agent_output;
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
                "no dunder.toml found; run 'dunder build file.ddr' or create a manifest".into(),
            );
        }
    }
}

fn publish(path: &Path, image: &[u8]) -> io::Result<()> {
    publish_file(path, image, 0o755)
}

fn publish_file(path: &Path, image: &[u8], mode: u32) -> io::Result<()> {
    write_atomic(path, image, mode, true)
}

fn write_atomic(path: &Path, image: &[u8], mode: u32, durable: bool) -> io::Result<()> {
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
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        if durable {
            f.sync_all()?;
        }
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn guard_sources(path: &Path, sources: &dunder::modules::Sources) -> Result<(), String> {
    if let Ok(target) = fs::metadata(path) {
        for source in &sources.files {
            if let Ok(input) = fs::metadata(&source.path) {
                if target.dev() == input.dev() && target.ino() == input.ino() {
                    return Err(format!(
                        "output must not overwrite the source file: {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn bundle_identifier(name: &str) -> String {
    let mut sanitized = String::new();
    let mut hash = 0x811c9dc5u32;
    for byte in name.bytes() {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x01000193);
        if byte.is_ascii_alphanumeric() {
            sanitized.push(char::from(byte.to_ascii_lowercase()));
        } else if !sanitized.is_empty() && !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_end_matches('-');
    let sanitized = if sanitized.is_empty() {
        "app"
    } else {
        sanitized
    };
    format!("org.dunder.{sanitized}-{hash:08x}")
}

struct AppBundle {
    contents: PathBuf,
    executable: PathBuf,
    info_path: PathBuf,
    resources_path: PathBuf,
    identifier: String,
    info: String,
}

impl AppBundle {
    fn new(output: &Path) -> Result<Self, String> {
        let name = output
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".app"))
            .filter(|name| {
                !name.is_empty()
                    && !matches!(*name, "." | "..")
                    && !name.chars().any(char::is_control)
            })
            .ok_or("--app output must end in a nonempty name followed by '.app'")?;
        let contents = output.join("Contents");
        let executable = contents.join("MacOS").join(name);
        let info_path = contents.join("Info.plist");
        let resources_path = contents.join("_CodeSignature/CodeResources");
        let identifier = bundle_identifier(name);
        let name = xml(name);
        let version = env!("CARGO_PKG_VERSION");
        let info = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n\
             <key>CFBundleExecutable</key><string>{name}</string>\n\
             <key>CFBundleName</key><string>{name}</string>\n\
             <key>CFBundleDisplayName</key><string>{name}</string>\n\
             <key>CFBundleIdentifier</key><string>{identifier}</string>\n\
             <key>CFBundlePackageType</key><string>APPL</string>\n\
             <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>\n\
             <key>CFBundleVersion</key><string>{version}</string>\n\
             <key>CFBundleShortVersionString</key><string>{version}</string>\n\
             <key>NSHighResolutionCapable</key><true/>\n\
             <key>LSMinimumSystemVersion</key><string>11.0</string>\n\
             <key>NSSupportsAutomaticGraphicsSwitching</key><true/>\n\
             </dict>\n</plist>\n"
        );
        Ok(Self {
            contents,
            executable,
            info_path,
            resources_path,
            identifier,
            info,
        })
    }

    fn resources(&self) -> Result<Vec<u8>, String> {
        let bundle = self.contents.parent().unwrap();
        match fs::symlink_metadata(bundle) {
            Ok(metadata) if metadata.is_dir() => {
                for entry in fs::read_dir(bundle).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if entry.file_name() != "Contents" {
                        return Err(format!(
                            "unsupported bundle entry outside Contents: {}",
                            entry.path().display()
                        ));
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(format!("bundle must be a directory: {}", bundle.display())),
        }
        let mut files = Vec::new();
        self.collect_resources(&self.contents, &mut files)?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut resources = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n\
             <key>files</key><dict/>\n<key>files2</key><dict>\n",
        );
        for (name, hash) in files {
            resources.push_str(&format!(
                "<key>{}</key><dict><key>hash2</key><data>{}</data></dict>\n",
                xml(&name),
                base64(&hash)
            ));
        }
        resources.push_str(
            "</dict>\n<key>rules</key><dict/>\n\
             <key>rules2</key><dict>\n\
             <key>^.*</key><true/>\n\
             <key>^Info\\.plist$</key><dict><key>omit</key><true/><key>weight</key><real>20</real></dict>\n\
             </dict>\n</dict>\n</plist>\n",
        );
        Ok(resources.into_bytes())
    }

    fn collect_resources(
        &self,
        directory: &Path,
        files: &mut Vec<(String, [u8; 32])>,
    ) -> Result<(), String> {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            _ => {
                return Err(format!(
                    "bundle resource directories must be real directories: {}",
                    directory.display()
                ))
            }
        }
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path == self.executable || path == self.info_path || path == self.resources_path {
                continue;
            }
            if directory == self.resources_path.parent().unwrap() {
                return Err(format!(
                    "unsupported existing bundle signature file: {}",
                    path.display()
                ));
            }
            let kind = fs::symlink_metadata(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .file_type();
            if kind.is_dir() {
                self.collect_resources(&path, files)?;
            } else if kind.is_file() {
                let name = path
                    .strip_prefix(&self.contents)
                    .unwrap()
                    .to_str()
                    .filter(|name| !name.chars().any(char::is_control))
                    .ok_or_else(|| {
                        format!("unsupported bundle resource filename: {}", path.display())
                    })?;
                let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                files.push((name.to_owned(), dunder::native::sha256(&bytes)));
            } else {
                return Err(format!(
                    "bundle resources must be regular files or directories: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        for index in [first >> 2, ((first & 3) << 4) | (second >> 4)] {
            result.push(char::from(ALPHABET[index as usize]));
        }
        result.push(if chunk.len() > 1 {
            char::from(ALPHABET[(((second & 15) << 2) | (third >> 6)) as usize])
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            char::from(ALPHABET[(third & 63) as usize])
        } else {
            '='
        });
    }
    result
}

fn run() -> Result<i32, String> {
    use dunder::target::Target;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        println!("Dunder — typed source to native executables\n\n  dunder build [file.ddr] [-o output] [--incremental] [--target target] [--app] [--timings]\n  dunder run [file.ddr] [-o output] [--incremental] [--target target] [--app] [--timings] [-- program-args...]\n  dunder check [file.ddr] [--agent]\n  dunder test [file.ddr] [-o directory] [--target target] [--timeout-ms 5000] [--affected]\n  dunder --list-targets\n\nWithout a file, finds the nearest dunder.toml. Default target: current host.\nCross-compilation needs no external toolchain or target SDK.\nrun and test require a target matching the current host.\n--app writes a macOS .app bundle and requires target arm64-macos; -o must name a .app directory.\nDeclarations: x := 42, x: i32 = 42, var x := 0. Values are immutable by default.");
        return Ok(0);
    }
    if args[0] == "--list-targets" && args.len() == 1 {
        for target in Target::ALL {
            println!("{target}");
        }
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
    let mut app = false;
    let mut agent = false;
    let mut affected = false;
    let mut incremental = false;
    let mut timeout = 5000;
    let mut program_args = Vec::new();
    let mut target = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--" && command == "run" {
            program_args = args[i + 1..].to_vec();
            break;
        }
        match args[i].as_str() {
            "-o" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("-o requires a path")?));
            }
            "--agent" if command == "check" => agent = true,
            "--affected" if command == "test" => affected = true,
            "--incremental" if matches!(command.as_str(), "build" | "run") => incremental = true,
            "--timings" => timings = true,
            "--target" => {
                i += 1;
                let value = args.get(i).ok_or("--target requires a target name")?;
                if target.is_some() {
                    return Err("--target may only be specified once".into());
                }
                target = Some(value.parse::<Target>()?);
            }
            "--app" => {
                if !matches!(command.as_str(), "build" | "run") {
                    return Err("--app is only supported by build and run".into());
                }
                if app {
                    return Err("--app may only be specified once".into());
                }
                app = true;
            }
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
    // Checking source does not need a supported native host.
    let target = if command == "check" {
        target
    } else {
        let selected = target.map(Ok).unwrap_or_else(Target::host)?;
        if app && selected != Target::Arm64Macos {
            return Err(format!(
                "--app requires target arm64-macos; selected target is {selected}"
            ));
        }
        if matches!(command.as_str(), "run" | "test") && !selected.can_run_on_host() {
            return Err(format!(
                "cannot {command} target {selected} on {}-{}; use 'dunder build --target {selected}' and run the binary on its target system",
                env::consts::ARCH, env::consts::OS
            ));
        }
        Some(selected)
    };
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
    let output = output.unwrap_or_else(|| {
        let name = if app {
            format!("{name}.app")
        } else if target == Some(Target::Wasm32) {
            format!("{name}.wasm")
        } else {
            name
        };
        let directory = PathBuf::from("build").join(target.map_or("", Target::name));
        let directory = if command == "test" {
            directory.join("tests")
        } else {
            directory
        };
        directory.join(name)
    });
    let bundle = if app {
        Some(AppBundle::new(&output)?)
    } else {
        None
    };
    let report = |error: dunder::modules::Diagnostic| {
        if agent {
            agent_output::diagnostic(&error, &source_path);
        }
        error.render()
    };
    let loaded = match dunder::modules::load(&source_path) {
        Ok(loaded) => loaded,
        Err(e) if agent => {
            report(e);
            return Ok(1);
        }
        Err(e) => return Err(report(e)),
    };
    let module_count = loaded.sources.files.len();
    let load_time = start.elapsed();
    let mut bindings = Vec::new();
    let checked = if agent {
        loaded.check_with_bindings(&mut bindings, target)
    } else if command == "test" {
        loaded.check_tests_for(target.unwrap())
    } else {
        match target {
            Some(target) => loaded.check_for(target),
            None => loaded.check(),
        }
    };
    let checked = match checked {
        Ok(checked) => checked,
        Err(e) if agent => {
            report(e);
            return Ok(1);
        }
        Err(e) => return Err(report(e)),
    };
    let check_time = start.elapsed() - load_time;
    if command == "test" {
        let target = target.unwrap();
        let result = test_runner::run(
            &checked,
            &output,
            Duration::from_millis(timeout),
            target,
            affected,
        )?;
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
        if agent {
            agent_output::success(&checked, &bindings);
        } else {
            println!("checked {} ({module_count} modules)", source_path.display());
        }
        return Ok(0);
    }
    let target = target.unwrap();
    let mut image = if incremental {
        let mut cache = dunder::native::Incremental::new(
            Path::new("build/.dunder-cache/functions"),
            &checked.program,
            target,
        )?;
        let image = cache.compile(checked.program.main.unwrap())?;
        if timings {
            eprintln!(
                "functions-reused={} functions-compiled={}",
                cache.stats.hits, cache.stats.misses
            );
        }
        image
    } else {
        checked
            .compile_for(target)
            .map_err(|e| checked.render(&e))?
    };
    let native_time = start.elapsed() - load_time - check_time;
    let mut resources = None;
    let executable = if let Some(bundle) = &bundle {
        // Validate every owned destination before publishing any file.
        guard_sources(&bundle.executable, &checked.sources)?;
        guard_sources(&bundle.info_path, &checked.sources)?;
        guard_sources(&bundle.resources_path, &checked.sources)?;
        let envelope = bundle.resources()?;
        dunder::native::sign_bundle(
            &mut image,
            &bundle.identifier,
            bundle.info.as_bytes(),
            &envelope,
        )?;
        resources = Some(envelope);
        &bundle.executable
    } else {
        guard_sources(&output, &checked.sources)?;
        &output
    };
    if target == Target::Wasm32 {
        publish_file(executable, &image, 0o644)
    } else {
        publish(executable, &image)
    }
    .map_err(|e| format!("{}: {e}", executable.display()))?;
    if let Some(bundle) = &bundle {
        publish_file(&bundle.info_path, bundle.info.as_bytes(), 0o644)
            .map_err(|e| format!("{}: {e}", bundle.info_path.display()))?;
        publish_file(&bundle.resources_path, resources.as_ref().unwrap(), 0o644)
            .map_err(|e| format!("{}: {e}", bundle.resources_path.display()))?;
    }
    if command == "run" {
        eprintln!("built {}", output.display());
    } else {
        println!("built {}", output.display());
    }
    if timings {
        eprintln!(
            "target={target} modules={module_count} load+parse={:.3}ms check={:.3}ms native={:.3}ms total={:.3}ms",
            load_time.as_secs_f64() * 1000.,
            check_time.as_secs_f64() * 1000.,
            native_time.as_secs_f64() * 1000.,
            start.elapsed().as_secs_f64() * 1000.
        );
    }
    if command == "run" {
        let executable = fs::canonicalize(executable).map_err(|e| e.to_string())?;
        let status = process::Command::new(executable)
            .args(&program_args)
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
