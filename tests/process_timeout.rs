#![cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "x86_64")
    )
))]

use std::{fs, path::Path, process::Command, time::Duration};

fn descendant_timeout(compiler: &Path, work: &Path) {
    fs::write(
        work.join("fork.ddr"),
        r#"
capability unsafe, filesystem
import sys
@extern("c", "fork") unsafe fn fork() -> i32
@extern("c", "usleep") unsafe fn sleep(interval: u32) -> i32
@extern("c", "_exit") unsafe fn exit(code: i32)
@test fn descendant() {
    unsafe {
        pid := fork()
        assert(pid >= 0)
        if pid == 0 {
            print("descendant-ready")
            paused := sleep(1200000)
            written := sys.write_text("escaped.txt", "escaped")
            exit(0)
        }
        while true { paused := sleep(10000) }
    }
}
"#,
    )
    .unwrap();
    let result = Command::new(compiler)
        .args(["test", "fork.ddr", "--timeout-ms", "300"])
        .current_dir(work)
        .output()
        .unwrap();
    let output = String::from_utf8_lossy(&result.stdout);
    assert_eq!(
        result.status.code(),
        Some(1),
        "{output}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        output.contains("descendant-ready"),
        "the forked child must actually run: {output}"
    );
    assert!(
        output.contains("timeout") || output.contains("exit 124"),
        "{output}"
    );
    std::thread::sleep(Duration::from_millis(1300));
    assert!(
        !work.join("escaped.txt").exists(),
        "timed-out descendant remained alive"
    );
}

#[test]
fn bootstrap_timeout_terminates_forked_descendant() {
    let work = std::env::temp_dir().join(format!("dunder-timeout-rust-{}", std::process::id()));
    fs::create_dir(&work).unwrap();
    descendant_timeout(Path::new(env!("CARGO_BIN_EXE_dunder")), &work);
    fs::remove_dir_all(work).unwrap();
}

#[test]
fn selfhost_timeout_terminates_forked_descendant() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = std::env::temp_dir().join(format!("dunder-timeout-selfhost-{}", std::process::id()));
    fs::create_dir(&work).unwrap();
    let entry = if cfg!(target_os = "macos") {
        "selfhost/main.ddr"
    } else if cfg!(target_arch = "aarch64") {
        "selfhost/linux.ddr"
    } else {
        "selfhost/x86_linux.ddr"
    };
    let mut compiler = Path::new(env!("CARGO_BIN_EXE_dunder")).to_path_buf();
    for stage in ["stage1", "stage2"] {
        let output = work.join(stage);
        let result = Command::new(&compiler)
            .args(["build", entry, "-o"])
            .arg(&output)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        compiler = output;
    }
    descendant_timeout(&compiler, &work);
    fs::remove_dir_all(work).unwrap();
}
