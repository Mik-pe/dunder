use dunder::target::Target;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-targets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("main.ddr"), "fn main(){print(42)}").unwrap();
        fs::write(
            path.join("dunder.toml"),
            "name = \"demo\"\nroot = \"main.ddr\"\n",
        )
        .unwrap();
        Self(path)
    }
    fn cli(&self, args: &[&str]) -> Output {
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

fn u16le(image: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(image[offset..offset + 2].try_into().unwrap())
}
fn u32le(image: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap())
}
fn u64le(image: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(image[offset..offset + 8].try_into().unwrap())
}

fn assert_target_image(image: &[u8], target: Target) {
    match target {
        Target::Wasm32 => assert_eq!(&image[..8], b"\0asm\x01\0\0\0"),
        Target::Arm64Macos => {
            assert_eq!(&image[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
            assert_eq!(u32le(image, 4), 0x0100_000c); // CPU_TYPE_ARM64
            assert_eq!(u32le(image, 12), 2); // MH_EXECUTE
        }
        Target::Arm64Linux | Target::X86_64Linux => {
            assert_eq!(&image[..7], b"\x7fELF\x02\x01\x01");
            assert_eq!(u16le(image, 16), 2); // ET_EXEC
            assert_eq!(
                u16le(image, 18),
                if target == Target::Arm64Linux {
                    183
                } else {
                    62
                }
            );
            let entry = u64le(image, 24);
            let table = u64le(image, 32) as usize;
            let stride = u16le(image, 54) as usize;
            let count = u16le(image, 56) as usize;
            assert!(stride >= 56);
            assert!(table + count * stride <= image.len());
            let mut executable_entry = false;
            for i in 0..count {
                let offset = table + i * stride;
                let kind = u32le(image, offset);
                assert_ne!(kind, 2, "a standalone image must not need dynamic linking");
                assert_ne!(kind, 3, "a standalone image must not need an interpreter");
                if kind == 1 {
                    // PT_LOAD
                    let flags = u32le(image, offset + 4);
                    let file_offset = u64le(image, offset + 8);
                    let address = u64le(image, offset + 16);
                    let file_size = u64le(image, offset + 32);
                    let memory_size = u64le(image, offset + 40);
                    assert!(file_offset + file_size <= image.len() as u64);
                    assert!(file_size <= memory_size);
                    if flags & 1 != 0 && entry >= address && entry < address + file_size {
                        executable_entry = true;
                    }
                }
            }
            assert!(
                executable_entry,
                "entry point must lie in executable file bytes"
            );
        }
    }
}

#[test]
fn every_target_builds_reproducibly_without_host_toolchains() {
    let work = Workspace::new();
    let source = fs::read_to_string(work.0.join("main.ddr")).unwrap();
    for target in [Target::Arm64Macos, Target::Arm64Linux, Target::X86_64Linux] {
        let direct = dunder::compile_for(&source, target).unwrap();
        assert_target_image(&direct, target);
        assert_eq!(direct, dunder::compile_for(&source, target).unwrap());
        let result = work.cli(&["build", "--target", target.name()]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let artifact = work.0.join("build").join(target.name()).join("demo");
        assert_eq!(fs::read(artifact).unwrap(), direct);
    }
}

#[test]
fn unsupported_and_incomplete_target_options_preserve_existing_outputs() {
    let work = Workspace::new();
    let output = work.0.join("program");
    fs::write(&output, b"previous output").unwrap();
    for args in [
        vec!["build", "--target", "sparc-linux", "-o", "program"],
        vec!["build", "-o", "program", "--target"],
    ] {
        let result = work.cli(&args);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("target"));
        assert_eq!(fs::read(&output).unwrap(), b"previous output");
    }
}

#[test]
fn foreign_run_and_test_fail_before_publishing_or_executing() {
    let work = Workspace::new();
    let output = work.0.join("program");
    fs::write(&output, b"previous output").unwrap();
    fs::write(
        work.0.join("main.ddr"),
        "@test fn probe(){assert(true)} fn main(){print(42)}",
    )
    .unwrap();
    for target in [Target::Arm64Macos, Target::Arm64Linux, Target::X86_64Linux] {
        if target.can_run_on_host() {
            continue;
        }
        for command in ["run", "test"] {
            let result = work.cli(&[
                command,
                "main.ddr",
                "--target",
                target.name(),
                "-o",
                "program",
            ]);
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(target.name()));
            assert_eq!(fs::read(&output).unwrap(), b"previous output");
        }
    }
}

#[test]
fn default_target_runs_with_empty_tool_path_and_no_dynamic_loader() {
    let work = Workspace::new();
    let host = Target::host().unwrap();
    let result = work.cli(&["run"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"42\n");
    let image = fs::read(work.0.join("build").join(host.name()).join("demo")).unwrap();
    assert_target_image(&image, host);
}
