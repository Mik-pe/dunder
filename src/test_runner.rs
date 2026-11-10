use dunder::modules::Checked;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Scratch(PathBuf);
impl Scratch {
    fn create(base: &Path) -> Result<Self, String> {
        fs::create_dir_all(base).map_err(|e| e.to_string())?;
        for i in 0..128 {
            let dir = base.join(format!("run-{}-{i}", std::process::id()));
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(Self(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("could not create a fresh test directory".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn capture(path: &Path) -> String {
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(4096).read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn execute(
    path: &Path,
    stdout: &Path,
    stderr: &Path,
    timeout: Duration,
) -> Result<(ExitStatus, bool), String> {
    let mut child = Command::new(path)
        .stdin(Stdio::null())
        .stdout(fs::File::create(stdout).map_err(|e| e.to_string())?)
        .stderr(fs::File::create(stderr).map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, false)),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let status = child.wait().map_err(|e| e.to_string())?;
            return Ok((status, true));
        }
        let elapsed = start.elapsed();
        let poll = if elapsed < Duration::from_millis(10) {
            Duration::from_micros(250)
        } else {
            Duration::from_millis(5)
        };
        thread::sleep(poll.min(timeout.saturating_sub(elapsed)));
    }
}

pub fn run(
    project: &Checked,
    output: &Path,
    timeout: Duration,
    target: dunder::target::Target,
    affected: bool,
) -> Result<i32, String> {
    let scratch = Scratch::create(output)?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut cache = affected
        .then(|| {
            dunder::native::Incremental::new(&output.join(".functions"), &project.program, target)
        })
        .transpose()?;
    let mut skipped = 0;
    let mut passed = 0;
    let mut failed = 0;
    for (index, test) in project.program.tests.iter().enumerate() {
        let receipt = output.join(".affected").join(format!("test-{index}"));
        let key = cache
            .as_ref()
            .and_then(|cache| cache.test_key(test.function))
            .map(|digest| {
                // A stricter timeout must exercise even a previously passing test.
                let mut input = digest.to_vec();
                input.extend_from_slice(&timeout.as_nanos().to_le_bytes());
                dunder::native::sha256(&input)
            });
        if let Some(key) = &key {
            if fs::read(&receipt).is_ok_and(|saved| saved == key.as_slice()) {
                skipped += 1;
                continue;
            }
            match fs::remove_file(&receipt) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let path = scratch.0.join(format!("test-{index}"));
        let image = if let Some(cache) = &mut cache {
            cache.compile(test.function)?
        } else {
            dunder::native::compile_entry_for(&project.program, test.function, target)?
        };
        super::publish(&path, &image).map_err(|e| e.to_string())?;
        let executable = fs::canonicalize(&path).map_err(|e| e.to_string())?;
        let stdout = scratch.0.join(format!("stdout-{index}"));
        let stderr = scratch.0.join(format!("stderr-{index}"));
        let (status, timed_out) = execute(&executable, &stdout, &stderr, timeout)?;
        let source = &project.sources.files[test.file].path;
        let source = source.strip_prefix(&cwd).unwrap_or(source);
        if status.success() && !timed_out {
            if let Some(key) = key {
                super::publish_file(&receipt, &key, 0o600).map_err(|e| e.to_string())?;
            }
            passed += 1;
            println!("PASS {}::{}", source.display(), test.name);
        } else {
            failed += 1;
            let reason = if timed_out {
                "timeout".into()
            } else {
                match status.code() {
                    Some(101) => "integer overflow".into(),
                    Some(102) => "division by zero".into(),
                    Some(103) => "index out of bounds".into(),
                    Some(104) => "assertion failed".into(),
                    Some(105) => "string arena exhausted".into(),
                    Some(106) => "invalid Unicode boundary or scalar".into(),
                    Some(107) => "floating conversion out of range".into(),
                    Some(108) => "invalid enum value".into(),
                    Some(109) => "dynamic storage exhausted".into(),
                    Some(110) => "stack exhausted".into(),
                    Some(n) => format!("exit {n}"),
                    None => "terminated by signal".into(),
                }
            };
            println!("FAIL {}::{} ({reason})", source.display(), test.name);
            for (label, output) in [("stdout", capture(&stdout)), ("stderr", capture(&stderr))] {
                if !output.is_empty() {
                    println!("  {label}: {}", output.trim_end());
                }
            }
        }
    }
    if affected {
        println!("{skipped} unaffected");
    }
    println!("{passed} passed, {failed} failed");
    Ok(if failed == 0 { 0 } else { 1 })
}
